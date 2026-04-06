//! MCP request handlers: initialize, list_skills, get_skill_info, run_skill.

use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

use crate::Error;
use crate::Result;

use skilllite_core::skill::manifest::{self, SkillIntegrityStatus};
use skilllite_core::skill::metadata;
use skilllite_core::skill::trust::TrustDecision;
use skilllite_sandbox::runner::SandboxLevel;
use skilllite_sandbox::security::types::{
    ScanResult, SecurityIssue, SecurityIssueType, SecuritySeverity,
};
use skilllite_sandbox::security::{run_skill_precheck, SKILL_PRECHECK_CRITICAL_BLOCKED};

use super::scan::format_l3_skill_precheck_response;
use super::state::{ConfirmedSkill, McpServer};

/// Handle the `initialize` request.
pub(super) fn handle_initialize(_params: &Value) -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "skilllite-mcp-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// Handle the `list_skills` tool call.
pub(super) fn handle_list_skills(server: &McpServer) -> Result<String> {
    let skills_dir = &server.skills_dir;
    if !skills_dir.exists() {
        return Ok("No skills directory found. Use `skilllite add` to install skills.".to_string());
    }

    let mut skills = Vec::new();

    // Scan subdirectories for skills
    let entries = std::fs::read_dir(skills_dir).map_err(|e| {
        Error::with_context(
            format!("Failed to read skills directory: {}", skills_dir.display()),
            e,
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        match metadata::parse_skill_metadata(&path) {
            Ok(meta) => {
                let lang = meta.language.as_deref().unwrap_or("auto");
                let desc = meta.description.as_deref().unwrap_or("No description");
                skills.push(json!({
                    "name": meta.name,
                    "description": desc,
                    "language": lang
                }));
            }
            Err(e) => {
                tracing::warn!("Failed to parse skill at {}: {}", path.display(), e);
            }
        }
    }

    if skills.is_empty() {
        return Ok("No skills installed. Use `skilllite add` to install skills.".to_string());
    }

    let result = json!({
        "skills": skills,
        "count": skills.len()
    });

    Ok(serde_json::to_string_pretty(&result)?)
}

/// Handle the `get_skill_info` tool call.
pub(super) fn handle_get_skill_info(server: &McpServer, arguments: &Value) -> Result<String> {
    let skill_name = arguments
        .get("skill_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("skill_name is required"))?;

    let skill_dir = server.skills_dir.join(skill_name);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        return Err(Error::msg(format!(
            "Skill '{}' not found in {}",
            skill_name,
            server.skills_dir.display()
        )));
    }

    let skill_content = std::fs::read_to_string(&skill_md_path).map_err(|e| {
        Error::with_context(format!("Failed to read SKILL.md for '{}'", skill_name), e)
    })?;

    // Also parse metadata for structured info
    let meta = metadata::parse_skill_metadata(&skill_dir)?;

    // Check for multi-script tools
    let scripts_dir = skill_dir.join("scripts");
    let mut scripts = Vec::new();
    if scripts_dir.exists() && scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if (fname.ends_with(".py")
                    || fname.ends_with(".js")
                    || fname.ends_with(".ts")
                    || fname.ends_with(".sh"))
                    && !fname.starts_with("test_")
                    && !fname.ends_with("_test.py")
                    && !fname.starts_with('.')
                    && fname != "__init__.py"
                {
                    scripts.push(fname);
                }
            }
        }
    }

    let mut output = format!("# Skill: {}\n\n", skill_name);
    output.push_str(&skill_content);

    if !scripts.is_empty() {
        output.push_str("\n\n## Available Scripts\n\n");
        for script in &scripts {
            output.push_str(&format!("- `scripts/{}`\n", script));
        }
    }

    // Include input schema if available
    if !meta.entry_point.is_empty() {
        let entry_path = skill_dir.join(&meta.entry_point);
        if entry_path.extension().and_then(|e| e.to_str()) == Some("py") {
            if let Some(schema) = parse_argparse_schema_from_path(&entry_path) {
                output.push_str(&format!(
                    "\n\n## Input Schema\n\n```json\n{}\n```\n",
                    serde_json::to_string_pretty(&schema)?
                ));
            }
        }
    }

    Ok(output)
}

/// Parse argparse schema from a Python file path (reuses agent/skills.rs logic).
fn parse_argparse_schema_from_path(script_path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(script_path).ok()?;

    let arg_re = regex::Regex::new(
        r#"\.add_argument\s*\(\s*['"]([^'"]+)['"](?:\s*,\s*['"]([^'"]+)['"])?([^)]*)\)"#,
    )
    .ok()?;
    let re_help = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#).ok();
    let re_type = regex::Regex::new(r"type\s*=\s*(\w+)").ok();

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for caps in arg_re.captures_iter(&content) {
        let arg_name = caps.get(1)?.as_str();
        let second_arg = caps.get(2).map(|m| m.as_str());
        let kwargs_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let (param_name, is_positional) = if let Some(stripped) = arg_name.strip_prefix("--") {
            (stripped.replace('-', "_"), false)
        } else if let Some(stripped) = arg_name.strip_prefix('-') {
            if let Some(s) = second_arg {
                if let Some(s2) = s.strip_prefix("--") {
                    (s2.replace('-', "_"), false)
                } else {
                    (stripped.to_string(), false)
                }
            } else {
                (stripped.to_string(), false)
            }
        } else {
            (arg_name.replace('-', "_"), true)
        };

        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), json!("string"));

        if let Some(help_cap) = re_help.as_ref().and_then(|re| re.captures(kwargs_str)) {
            prop.insert(
                "description".to_string(),
                json!(help_cap.get(1).map(|m| m.as_str()).unwrap_or("")),
            );
        }

        if let Some(type_cap) = re_type.as_ref().and_then(|re| re.captures(kwargs_str)) {
            match type_cap.get(1).map(|m| m.as_str()).unwrap_or("") {
                "int" => {
                    prop.insert("type".to_string(), json!("integer"));
                }
                "float" => {
                    prop.insert("type".to_string(), json!("number"));
                }
                "bool" => {
                    prop.insert("type".to_string(), json!("boolean"));
                }
                _ => {}
            }
        }

        if kwargs_str.contains("store_true") || kwargs_str.contains("store_false") {
            prop.insert("type".to_string(), json!("boolean"));
        }

        if is_positional || kwargs_str.contains("required=True") {
            required.push(param_name.clone());
        }

        properties.insert(param_name, Value::Object(prop));
    }

    if properties.is_empty() {
        return None;
    }

    Some(json!({
        "type": "object",
        "properties": properties,
        "required": required
    }))
}

/// Handle the `run_skill` tool call.
pub(super) fn handle_run_skill(server: &mut McpServer, arguments: &Value) -> Result<String> {
    let skill_name = arguments
        .get("skill_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::msg("skill_name is required"))?;
    let input = arguments.get("input").cloned().unwrap_or(json!({}));
    let confirmed = arguments
        .get("confirmed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let scan_id = arguments.get("scan_id").and_then(|v| v.as_str());

    // Find the skill
    let skill_dir = server.skills_dir.join(skill_name);
    if !skill_dir.exists() || !skill_dir.join("SKILL.md").exists() {
        return Err(Error::msg(format!(
            "Skill '{}' not found in {}",
            skill_name,
            server.skills_dir.display()
        )));
    }
    let meta_early = metadata::parse_skill_metadata(&skill_dir)?;
    if let Some(msg) = skilllite_core::skill::denylist::deny_reason_for_skill_name(&meta_early.name)
    {
        return Err(Error::msg(msg));
    }
    let integrity = manifest::evaluate_skill_status(&server.skills_dir, &skill_dir)?;
    let block = skilllite_core::config::supply_chain_block_enabled();
    if block {
        match integrity.status {
            SkillIntegrityStatus::Ok | SkillIntegrityStatus::Unsigned => {}
            SkillIntegrityStatus::HashChanged => {
                return Err(Error::msg(
                    "Execution blocked: Skill fingerprint changed since installation. Please reinstall this skill before running.",
                ));
            }
            SkillIntegrityStatus::SignatureInvalid => {
                return Err(Error::msg(
                    "Execution blocked: Skill signature is invalid. Please verify source and reinstall.",
                ));
            }
        }
        match integrity.trust_decision {
            TrustDecision::Deny => {
                return Err(Error::msg(
                    "Execution blocked: Skill trust tier is Deny. Reinstall from trusted source or verify integrity.",
                ));
            }
            TrustDecision::RequireConfirm => {}
            TrustDecision::Allow => {}
        }
    } else if matches!(
        integrity.status,
        SkillIntegrityStatus::HashChanged | SkillIntegrityStatus::SignatureInvalid
    ) {
        tracing::warn!(
            skill = %skill_dir.display(),
            status = ?integrity.status,
            "Skill integrity issue (P0 observable mode: execution allowed; set SKILLLITE_SUPPLY_CHAIN_BLOCK=1 to block)"
        );
    }

    let meta = meta_early;
    let sandbox_level = SandboxLevel::from_env_or_cli(None);

    // Level 3: unified SKILL.md + entry script precheck (same as CLI runner / agent policy).
    // MCP is non-interactive: never rely on runner stdin; gate here with scan_id + skip runner precheck.
    if sandbox_level == SandboxLevel::Level3 {
        let code_hash = McpServer::compute_skill_hash(&skill_dir, &meta.entry_point);

        let already_confirmed = server
            .confirmed_skills
            .get(skill_name)
            .is_some_and(|c| c.code_hash == code_hash);

        if !already_confirmed {
            if confirmed {
                let sid = scan_id.ok_or_else(|| {
                    Error::msg(
                        "scan_id is required when confirmed=true. Run run_skill once without confirmed to obtain a fresh Level-3 skill precheck.",
                    )
                })?;

                let cached = server.scan_cache.remove(sid).ok_or_else(|| {
                    Error::msg(
                        "Invalid or expired scan_id. Run run_skill again to refresh the Level-3 skill precheck.",
                    )
                })?;

                if !cached.is_l3_skill_precheck {
                    return Err(Error::msg(
                        "This scan_id is not a Level-3 skill precheck token. Call run_skill without confirmed to run the unified SKILL.md + entry scan.",
                    ));
                }

                let current_hash = McpServer::compute_skill_hash(&skill_dir, &meta.entry_point);
                if cached.code_hash != current_hash {
                    return Err(Error::msg(
                        "Stale scan_id: skill content changed since this precheck was issued.",
                    ));
                }

                if cached.l3_script_critical {
                    return Err(Error::msg(SKILL_PRECHECK_CRITICAL_BLOCKED.to_string()));
                }

                skilllite_core::observability::audit_confirmation_response(
                    skill_name, true, "user",
                );
                skilllite_core::observability::security_scan_approved(skill_name, sid, 1);

                server
                    .confirmed_skills
                    .insert(skill_name.to_string(), ConfirmedSkill { code_hash });
            } else {
                let summary =
                    run_skill_precheck(&skill_dir, &meta.entry_point, meta.network.enabled);

                if let Some(report) = summary.review_text {
                    let scan_token =
                        McpServer::generate_scan_id(&format!("l3sk:{}:{}", skill_name, code_hash));
                    let synthetic = ScanResult {
                        is_safe: false,
                        issues: vec![SecurityIssue {
                            rule_id: "l3-skill-precheck".to_string(),
                            severity: SecuritySeverity::High,
                            issue_type: SecurityIssueType::SystemAccess,
                            line_number: 0,
                            description: "Level-3 skill precheck: see report in this response."
                                .to_string(),
                            code_snippet: String::new(),
                        }],
                    };
                    server.scan_cache.insert(
                        scan_token.clone(),
                        super::state::CachedScan {
                            scan_result: synthetic,
                            code_hash: code_hash.clone(),
                            language: "l3_skill_precheck".to_string(),
                            code: report.clone(),
                            created_at: Instant::now(),
                            is_l3_skill_precheck: true,
                            l3_script_critical: summary.has_critical_script_issue,
                        },
                    );
                    skilllite_core::observability::audit_confirmation_requested(
                        skill_name,
                        &code_hash,
                        1,
                        "SKILL_STATIC_PRECHECK",
                    );
                    return format_l3_skill_precheck_response(
                        &report,
                        summary.has_critical_script_issue,
                        &scan_token,
                        &code_hash,
                    );
                }

                server
                    .confirmed_skills
                    .insert(skill_name.to_string(), ConfirmedSkill { code_hash });
            }
        }
    }

    // Execute the skill
    let input_json = serde_json::to_string(&input)?;

    if meta.entry_point.is_empty() {
        // Prompt-only skill or multi-script skill without entry_point
        return Ok(format!(
            "Skill '{}' has no entry point. It is a prompt-only skill or uses multi-script tools.\n\
             Use get_skill_info to see available scripts.",
            skill_name
        ));
    }

    if block && matches!(integrity.trust_decision, TrustDecision::RequireConfirm) && !confirmed {
        return Err(Error::msg(format!(
            "Execution blocked: Skill requires confirmation (trust tier: {:?}). Pass confirmed=true to run.",
            integrity.trust_tier
        )));
    }

    // Setup environment
    let cache_dir = skilllite_core::config::CacheConfig::cache_dir();
    let env_spec = skilllite_core::EnvSpec::from_metadata(&skill_dir, &meta);
    let env_path = skilllite_sandbox::env::builder::ensure_environment(
        &skill_dir,
        &env_spec,
        cache_dir.as_deref(),
        None,
        None,
    )?;

    let limits = skilllite_sandbox::runner::ResourceLimits::from_env();

    let runtime = skilllite_sandbox::env::builder::build_runtime_paths(&env_path);
    let config = skilllite_sandbox::runner::SandboxConfig {
        name: meta.name.clone(),
        entry_point: meta.entry_point.clone(),
        language: metadata::detect_language(&skill_dir, &meta),
        network_enabled: meta.network.enabled,
        network_outbound: meta.network.outbound.clone(),
        uses_playwright: meta.uses_playwright(),
    };
    let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level_opt(
        &skill_dir,
        &runtime,
        &config,
        &input_json,
        limits,
        sandbox_level,
        skilllite_sandbox::runner::SandboxRunOptions {
            skip_skill_precheck: matches!(sandbox_level, SandboxLevel::Level3),
        },
    )?;

    Ok(output)
}
