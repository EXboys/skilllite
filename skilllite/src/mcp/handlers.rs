//! MCP request handlers: initialize, list_skills, get_skill_info, run_skill.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use skilllite_sandbox::runner::SandboxLevel;
use skilllite_sandbox::security::types::SecuritySeverity;
use skilllite_core::skill::metadata;
use skilllite_core::skill::manifest::{self, SkillIntegrityStatus};
use skilllite_core::skill::trust::TrustDecision;

use super::state::{ConfirmedSkill, McpServer};
use super::scan::{perform_scan, format_scan_response};

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
    let entries = std::fs::read_dir(skills_dir)
        .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

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
        .context("skill_name is required")?;

    let skill_dir = server.skills_dir.join(skill_name);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        anyhow::bail!("Skill '{}' not found in {}", skill_name, server.skills_dir.display());
    }

    let skill_content = std::fs::read_to_string(&skill_md_path)
        .with_context(|| format!("Failed to read SKILL.md for '{}'", skill_name))?;

    // Also parse metadata for structured info
    let meta = metadata::parse_skill_metadata(&skill_dir)?;

    // Check for multi-script tools
    let scripts_dir = skill_dir.join("scripts");
    let mut scripts = Vec::new();
    if scripts_dir.exists() && scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".py") || fname.ends_with(".js") || fname.ends_with(".ts") || fname.ends_with(".sh") {
                    if !fname.starts_with("test_") && !fname.ends_with("_test.py") && !fname.starts_with('.') && fname != "__init__.py" {
                        scripts.push(fname);
                    }
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
    ).ok()?;

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for caps in arg_re.captures_iter(&content) {
        let arg_name = caps.get(1)?.as_str();
        let second_arg = caps.get(2).map(|m| m.as_str());
        let kwargs_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let (param_name, is_positional) = if arg_name.starts_with("--") {
            (arg_name[2..].replace('-', "_"), false)
        } else if arg_name.starts_with('-') {
            if let Some(s) = second_arg {
                if s.starts_with("--") {
                    (s[2..].replace('-', "_"), false)
                } else {
                    (arg_name[1..].to_string(), false)
                }
            } else {
                (arg_name[1..].to_string(), false)
            }
        } else {
            (arg_name.replace('-', "_"), true)
        };

        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), json!("string"));

        if let Some(help_cap) = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#)
            .ok().and_then(|re| re.captures(kwargs_str))
        {
            prop.insert("description".to_string(), json!(help_cap.get(1).unwrap().as_str()));
        }

        if let Some(type_cap) = regex::Regex::new(r"type\s*=\s*(\w+)")
            .ok().and_then(|re| re.captures(kwargs_str))
        {
            match type_cap.get(1).unwrap().as_str() {
                "int" => { prop.insert("type".to_string(), json!("integer")); }
                "float" => { prop.insert("type".to_string(), json!("number")); }
                "bool" => { prop.insert("type".to_string(), json!("boolean")); }
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
        .context("skill_name is required")?;
    let input = arguments
        .get("input")
        .cloned()
        .unwrap_or(json!({}));
    let confirmed = arguments
        .get("confirmed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let scan_id = arguments
        .get("scan_id")
        .and_then(|v| v.as_str());

    // Find the skill
    let skill_dir = server.skills_dir.join(skill_name);
    if !skill_dir.exists() || !skill_dir.join("SKILL.md").exists() {
        anyhow::bail!("Skill '{}' not found in {}", skill_name, server.skills_dir.display());
    }
    let integrity = manifest::evaluate_skill_status(&server.skills_dir, &skill_dir)?;
    match integrity.status {
        SkillIntegrityStatus::Ok | SkillIntegrityStatus::Unsigned => {}
        SkillIntegrityStatus::HashChanged => {
            anyhow::bail!(
                "Execution blocked: Skill fingerprint changed since installation. Please reinstall this skill before running."
            );
        }
        SkillIntegrityStatus::SignatureInvalid => {
            anyhow::bail!(
                "Execution blocked: Skill signature is invalid. Please verify source and reinstall."
            );
        }
    }
    // Trust tier enforcement
    match integrity.trust_decision {
        TrustDecision::Deny => {
            anyhow::bail!(
                "Execution blocked: Skill trust tier is Deny. Reinstall from trusted source or verify integrity."
            );
        }
        TrustDecision::RequireConfirm => {
            if !confirmed {
                anyhow::bail!(
                    "Execution blocked: Skill requires confirmation (trust tier: {:?}). Pass confirmed=true to run.",
                    integrity.trust_tier
                );
            }
        }
        TrustDecision::Allow => {}
    }

    let meta = metadata::parse_skill_metadata(&skill_dir)?;
    let sandbox_level = SandboxLevel::from_env_or_cli(None);

    // Security check for Level 3
    if sandbox_level == SandboxLevel::Level3 {
        let code_hash = McpServer::compute_skill_hash(&skill_dir, &meta.entry_point);

        // Check session-level confirmation cache
        let already_confirmed = server.confirmed_skills.get(skill_name)
            .map_or(false, |c| c.code_hash == code_hash);

        if !already_confirmed {
            if confirmed {
                // Verify scan_id
                let sid = scan_id.context(
                    "scan_id is required when confirmed=true. The skill security scan must be reviewed first."
                )?;

                let issues_count = {
                    let cached = server.scan_cache.get(sid).context(
                        "Invalid or expired scan_id. Please review the security report and try again."
                    )?;
                    let has_critical = cached.scan_result.issues.iter().any(|i| {
                        matches!(i.severity, SecuritySeverity::Critical)
                    });
                    if has_critical {
                        anyhow::bail!(
                            "Execution blocked: Critical security issues cannot be overridden."
                        );
                    }
                    cached.scan_result.issues.len()
                };

                // One-time consumption: remove scan_id to prevent replay (F4)
                server.scan_cache.remove(sid);

                // Audit: scan approved
                skilllite_core::observability::audit_confirmation_response(skill_name, true, "user");
                skilllite_core::observability::security_scan_approved(
                    skill_name,
                    sid,
                    issues_count,
                );

                // Cache confirmation
                server.confirmed_skills.insert(
                    skill_name.to_string(),
                    ConfirmedSkill { code_hash },
                );
            } else {
                // Perform security scan on entry point
                let entry_path = if !meta.entry_point.is_empty() {
                    skill_dir.join(&meta.entry_point)
                } else {
                    // Multi-script or no entry point — scan SKILL.md content as proxy
                    skill_dir.join("SKILL.md")
                };

                if entry_path.exists() {
                    let code = std::fs::read_to_string(&entry_path).unwrap_or_default();
                    let language = if entry_path.extension().and_then(|e| e.to_str()) == Some("py") {
                        "python"
                    } else if entry_path.extension().and_then(|e| e.to_str()) == Some("js") {
                        "javascript"
                    } else {
                        "bash"
                    };

                    let (scan_result, new_scan_id, new_code_hash) = perform_scan(server, language, &code)?;

                    let has_high = scan_result.issues.iter().any(|i| {
                        matches!(i.severity, SecuritySeverity::High | SecuritySeverity::Critical)
                    });

                    if has_high {
                        // Audit: confirmation requested
                        skilllite_core::observability::audit_confirmation_requested(
                            skill_name,
                            &new_code_hash,
                            scan_result.issues.len(),
                            "High",
                        );
                        skilllite_core::observability::security_scan_high(
                            skill_name,
                            "High",
                            &serde_json::json!(scan_result.issues.iter().map(|i| {
                                serde_json::json!({
                                    "rule": i.rule_id,
                                    "severity": format!("{:?}", i.severity),
                                    "description": i.description,
                                })
                            }).collect::<Vec<_>>()),
                        );
                        return format_scan_response(&scan_result, &new_scan_id, &new_code_hash);
                    }
                }

                // No high-severity issues — cache and proceed
                server.confirmed_skills.insert(
                    skill_name.to_string(),
                    ConfirmedSkill { code_hash },
                );
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

    // Setup environment
    let cache_dir = skilllite_core::config::CacheConfig::cache_dir();
    let env_path = skilllite_sandbox::env::builder::ensure_environment(
        &skill_dir,
        &meta,
        cache_dir.as_deref(),
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
    let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level(
        &skill_dir,
        &runtime,
        &config,
        &input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

