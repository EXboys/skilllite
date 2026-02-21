//! Skills invocation: wraps sandbox run/exec for the agent layer.
//!
//! Since Agent is in the same process as Sandbox, we call the sandbox
//! executor directly (no IPC needed). Ported from Python `ToolCallHandler`.
//!
//! Phase 2.5 additions:
//!   - Security scanning before skill execution (L3)
//!   - Multi-script skill support (skill_name__script_name)
//!   - Argparse schema inference for Python scripts
//!   - .skilllite.lock dependency resolution

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::sandbox::executor::{ResourceLimits, SandboxConfig, SandboxLevel};
use crate::sandbox::security::scanner::ScriptScanner;
use crate::skill::metadata::{self, SkillMetadata};

use super::types::{EventSink, ToolDefinition, FunctionDef, ToolResult};

/// A loaded skill ready for invocation.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub name: String,
    pub skill_dir: PathBuf,
    pub metadata: SkillMetadata,
    pub tool_definitions: Vec<ToolDefinition>,
    /// Multi-script tool mapping: tool_name → script_path (e.g. "scripts/init_skill.py")
    pub multi_script_entries: HashMap<String, String>,
}

/// Load skills from directories, parse SKILL.md, generate tool definitions.
pub fn load_skills(skill_dirs: &[String]) -> Vec<LoadedSkill> {
    let mut skills = Vec::new();

    for dir_path in skill_dirs {
        let path = Path::new(dir_path);
        if !path.exists() || !path.is_dir() {
            tracing::warn!("Skill directory not found: {}", dir_path);
            continue;
        }

        // Check if this directory itself is a skill (has SKILL.md)
        if path.join("SKILL.md").exists() {
            if let Some(skill) = load_single_skill(path) {
                skills.push(skill);
            }
        } else {
            // Scan subdirectories for skills
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() && entry_path.join("SKILL.md").exists() {
                        if let Some(skill) = load_single_skill(&entry_path) {
                            skills.push(skill);
                        }
                    }
                }
            }
        }
    }

    skills
}

/// Load a single skill from a directory.
fn load_single_skill(skill_dir: &Path) -> Option<LoadedSkill> {
    let metadata = match metadata::parse_skill_metadata(skill_dir) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to parse skill at {}: {}", skill_dir.display(), e);
            return None;
        }
    };

    let name = metadata.name.clone();
    let tool_name = sanitize_tool_name(&name);

    // Generate tool definition based on skill type
    let mut tool_defs = if metadata.is_bash_tool_skill() {
        // Bash-tool skill: command string parameter
        let patterns = metadata.get_bash_patterns();
        let desc = metadata.description.clone().unwrap_or_else(|| {
            format!("Execute commands for {}. Allowed patterns: {:?}", name, patterns)
        });
        vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: tool_name,
                description: desc,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": format!("Bash command to execute. Must match allowed patterns: {:?}", patterns)
                        }
                    },
                    "required": ["command"]
                }),
            },
        }]
    } else if !metadata.entry_point.is_empty() {
        // Regular skill with entry point — try argparse schema inference
        let desc = metadata.description.clone().unwrap_or_else(|| {
            format!("Execute skill: {}", name)
        });
        let schema = infer_entry_point_schema(skill_dir, &metadata)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": true
                })
            });
        vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: tool_name,
                description: desc,
                parameters: schema,
            },
        }]
    } else {
        // No single entry point — check for multi-script skill
        Vec::new()
    };

    // Phase 2.5: detect multi-script skills (scripts/ directory with multiple entry points)
    let mut multi_script_entries = HashMap::new();
    if tool_defs.is_empty() && !metadata.is_bash_tool_skill() {
        let (multi_tools, entries) = detect_multi_script_tools(skill_dir, &name);
        tool_defs.extend(multi_tools);
        multi_script_entries = entries;
    }

    Some(LoadedSkill {
        name,
        skill_dir: skill_dir.to_path_buf(),
        metadata,
        tool_definitions: tool_defs,
        multi_script_entries,
    })
}

// ─── Phase 2.5: Multi-script skill support ──────────────────────────────────

/// Detect multiple scripts in a skill's `scripts/` directory and generate
/// a separate tool definition for each.
/// Returns (tool_definitions, entry_map: tool_name → script_path).
/// Ported from Python `detect_all_scripts` + `analyze_multi_script_skill`.
fn detect_multi_script_tools(
    skill_dir: &Path,
    skill_name: &str,
) -> (Vec<ToolDefinition>, HashMap<String, String>) {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() || !scripts_dir.is_dir() {
        return (Vec::new(), HashMap::new());
    }

    let extensions = [
        (".py", "python"),
        (".js", "node"),
        (".ts", "node"),
        (".sh", "bash"),
    ];

    let skip_names = ["__init__.py"];
    let mut tools = Vec::new();
    let mut entries = HashMap::new();

    for (ext, _lang) in &extensions {
        if let Ok(dir_entries) = std::fs::read_dir(&scripts_dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                let fname = path.file_name().map(|n| n.to_string_lossy().to_string());
                let fname = match fname {
                    Some(f) => f,
                    None => continue,
                };

                if !fname.ends_with(ext) {
                    continue;
                }
                if fname.starts_with("test_")
                    || fname.ends_with("_test.py")
                    || fname.starts_with('.')
                    || skip_names.contains(&fname.as_str())
                {
                    continue;
                }

                let script_stem = fname.trim_end_matches(ext).replace('_', "-");
                // Tool name: skill_name__script_name (double underscore)
                let tool_name = format!(
                    "{}__{}",
                    sanitize_tool_name(skill_name),
                    sanitize_tool_name(&script_stem)
                );

                let script_path = format!("scripts/{}", fname);

                let desc = format!(
                    "Execute {} from skill '{}'",
                    script_path, skill_name
                );

                // Try argparse inference for Python scripts
                let schema = if fname.ends_with(".py") {
                    parse_argparse_schema(&path).unwrap_or_else(|| flexible_schema())
                } else {
                    flexible_schema()
                };

                // Store the mapping: tool_name → script_path
                entries.insert(tool_name.clone(), script_path);

                tools.push(ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDef {
                        name: tool_name,
                        description: desc,
                        parameters: schema,
                    },
                });
            }
        }
    }

    (tools, entries)
}

/// Return a flexible JSON schema that accepts any properties.
fn flexible_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

// ─── Phase 2.5: Argparse schema inference ───────────────────────────────────

/// Try to infer parameter schema from a skill's entry point script.
/// If the entry point is a Python file, parse argparse calls.
fn infer_entry_point_schema(skill_dir: &Path, metadata: &SkillMetadata) -> Option<serde_json::Value> {
    let entry = &metadata.entry_point;
    if entry.is_empty() {
        return None;
    }
    let script_path = skill_dir.join(entry);
    if script_path.extension().and_then(|e| e.to_str()) != Some("py") {
        return None;
    }
    parse_argparse_schema(&script_path)
}

/// Parse Python script for argparse `add_argument` calls and generate JSON schema.
/// Ported from Python `tool_builder.py` `_parse_argparse_schema`.
fn parse_argparse_schema(script_path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(script_path).ok()?;

    let arg_re = regex::Regex::new(
        r#"\.add_argument\s*\(\s*['"]([^'"]+)['"](?:\s*,\s*['"]([^'"]+)['"])?([^)]*)\)"#,
    )
    .ok()?;

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for caps in arg_re.captures_iter(&content) {
        let arg_name = caps.get(1)?.as_str();
        let second_arg = caps.get(2).map(|m| m.as_str());
        let kwargs_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        // Determine parameter name
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
        prop.insert("type".to_string(), serde_json::json!("string"));

        // Extract help text
        if let Some(help_cap) = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            prop.insert(
                "description".to_string(),
                serde_json::json!(help_cap.get(1).unwrap().as_str()),
            );
        }

        // Extract type
        if let Some(type_cap) = regex::Regex::new(r"type\s*=\s*(\w+)")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            match type_cap.get(1).unwrap().as_str() {
                "int" => {
                    prop.insert("type".to_string(), serde_json::json!("integer"));
                }
                "float" => {
                    prop.insert("type".to_string(), serde_json::json!("number"));
                }
                "bool" => {
                    prop.insert("type".to_string(), serde_json::json!("boolean"));
                }
                _ => {}
            }
        }

        // Check action=store_true/store_false
        if let Some(action_cap) = regex::Regex::new(r#"action\s*=\s*['"](\w+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let action = action_cap.get(1).unwrap().as_str();
            if action == "store_true" || action == "store_false" {
                prop.insert("type".to_string(), serde_json::json!("boolean"));
            }
        }

        // Check nargs
        if let Some(nargs_cap) = regex::Regex::new(r#"nargs\s*=\s*['"]?([^,\s)]+)['"]?"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let nargs = nargs_cap.get(1).unwrap().as_str();
            if nargs == "*" || nargs == "+" || nargs.parse::<u32>().is_ok() {
                prop.insert("type".to_string(), serde_json::json!("array"));
                prop.insert("items".to_string(), serde_json::json!({"type": "string"}));
            }
        }

        // Check choices
        if let Some(choices_cap) = regex::Regex::new(r"choices\s*=\s*\[([^\]]+)\]")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let choices_str = choices_cap.get(1).unwrap().as_str();
            let choices: Vec<String> = regex::Regex::new(r#"['"]([^'"]+)['"]"#)
                .ok()
                .map(|re| {
                    re.captures_iter(choices_str)
                        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if !choices.is_empty() {
                prop.insert("enum".to_string(), serde_json::json!(choices));
            }
        }

        // Check default
        if let Some(default_cap) = regex::Regex::new(r"default\s*=\s*([^,)]+)")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let val = default_cap.get(1).unwrap().as_str().trim();
            if val != "None" && val != "\"\"" && val != "''" {
                let cleaned = val.trim_matches(|c| c == '"' || c == '\'');
                prop.insert("default".to_string(), serde_json::json!(cleaned));
            }
        }

        // Check required
        let is_required = kwargs_str.contains("required=True") || is_positional;
        if is_required {
            required.push(param_name.clone());
        }

        properties.insert(param_name, serde_json::Value::Object(prop));
    }

    if properties.is_empty() {
        return None;
    }

    Some(serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required
    }))
}

/// Sanitize skill name to a valid tool function name.
/// Replaces non-alphanumeric chars with underscore, lowercases.
fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

/// Execute a skill tool call. Dispatches to sandbox execution.
/// Returns the tool result content.
pub fn execute_skill(
    skill: &LoadedSkill,
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> ToolResult {
    let result = execute_skill_inner(skill, tool_name, arguments, workspace, event_sink);
    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
        },
    }
}

// Session-level cache of confirmed skills (skill_name → code_hash).
// Avoids re-scanning skills that were already confirmed in this session.
// Thread-local because EventSink requires &mut (not shareable across threads).
thread_local! {
    static CONFIRMED_SKILLS: std::cell::RefCell<HashMap<String, String>> =
        std::cell::RefCell::new(HashMap::new());
}

fn execute_skill_inner(
    skill: &LoadedSkill,
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> Result<String> {
    let skill_dir = &skill.skill_dir;
    let metadata = &skill.metadata;

    // Phase 2.5: Multi-script tool routing
    // If tool_name is in the multi_script_entries map, use that script as entry_point.
    // Try exact match first, then normalized match (hyphens → underscores).
    let multi_script_entry: Option<&String> = skill
        .multi_script_entries
        .get(tool_name)
        .or_else(|| skill.multi_script_entries.get(&sanitize_tool_name(tool_name)));

    // For Level 3: security scan + user confirmation
    // Ported from Python `UnifiedExecutionService.execute_skill` L3 flow
    let sandbox_level = SandboxLevel::from_env_or_cli(None);
    if sandbox_level == SandboxLevel::Level3 {
        let code_hash = compute_skill_hash(skill_dir, metadata);

        // Check session-level confirmation cache
        let already_confirmed = CONFIRMED_SKILLS.with(|cache| {
            let cache = cache.borrow();
            cache.get(&skill.name).map_or(false, |h| h == &code_hash)
        });

        if !already_confirmed {
            // Run security scan on entry point
            let scan_report = run_security_scan(skill_dir, metadata);

            let prompt = if let Some(report) = scan_report {
                format!(
                    "Skill '{}' security scan results:\n\n{}\n\nAllow execution?",
                    skill.name, report
                )
            } else {
                format!(
                    "Skill '{}' wants to execute code (sandbox level 3). Allow?",
                    skill.name
                )
            };

            if !event_sink.on_confirmation_request(&prompt) {
                return Ok("Execution cancelled by user.".to_string());
            }

            // Cache confirmation
            CONFIRMED_SKILLS.with(|cache| {
                cache.borrow_mut().insert(skill.name.clone(), code_hash);
            });
        }
    }

    // Setup environment
    let cache_dir = crate::config::CacheConfig::cache_dir();
    let env_path = crate::env::builder::ensure_environment(
        skill_dir,
        metadata,
        cache_dir.as_deref(),
    )?;

    let limits = ResourceLimits::from_env();

    if metadata.is_bash_tool_skill() {
        // Bash-tool skill: extract command from arguments
        let args: Value = serde_json::from_str(arguments)
            .context("Invalid arguments JSON")?;
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .context("'command' is required for bash-tool skills")?;

        let skill_patterns = metadata.get_bash_patterns();
        let validator_patterns: Vec<crate::sandbox::bash_validator::BashToolPattern> = skill_patterns
            .into_iter()
            .map(|p| crate::sandbox::bash_validator::BashToolPattern {
                command_prefix: p.command_prefix,
                raw_pattern: p.raw_pattern,
            })
            .collect();
        crate::sandbox::bash_validator::validate_bash_command(command, &validator_patterns)
            .map_err(|e| anyhow::anyhow!("Command validation failed: {}", e))?;

        // Execute bash command (same logic as main.rs bash_command).
        // Resolve the effective cwd: prefer SKILLLITE_OUTPUT_DIR so file outputs
        // (screenshots, PDFs, etc.) land in the output directory automatically.
        let effective_cwd = crate::config::PathsConfig::from_env()
            .output_dir
            .as_ref()
            .map(|s| std::path::PathBuf::from(s))
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| workspace.to_path_buf());

        execute_bash_in_skill(skill_dir, command, &env_path, &effective_cwd, workspace)
    } else {
        // Regular skill or multi-script tool: pass arguments as input JSON
        let input_json = if arguments.trim().is_empty() || arguments.trim() == "{}" {
            "{}".to_string()
        } else {
            arguments.to_string()
        };

        // Validate input JSON
        let _: Value = serde_json::from_str(&input_json)
            .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

        let effective_metadata = if let Some(ref entry) = multi_script_entry {
            let mut m = metadata.clone();
            m.entry_point = entry.to_string();
            m
        } else {
            metadata.clone()
        };

        let runtime = crate::env::builder::build_runtime_paths(&env_path);
        let config = build_sandbox_config(skill_dir, &effective_metadata);
        let output = crate::sandbox::executor::run_in_sandbox_with_limits_and_level(
            skill_dir,
            &runtime,
            &config,
            &input_json,
            limits,
            sandbox_level,
        )?;
        Ok(output)
    }
}

/// Build a `SandboxConfig` from `SkillMetadata`, resolving language via `detect_language`.
fn build_sandbox_config(skill_dir: &Path, metadata: &SkillMetadata) -> SandboxConfig {
    SandboxConfig {
        name: metadata.name.clone(),
        entry_point: metadata.entry_point.clone(),
        language: metadata::detect_language(skill_dir, metadata),
        network_enabled: metadata.network.enabled,
        network_outbound: metadata.network.outbound.clone(),
        uses_playwright: metadata.uses_playwright(),
    }
}

/// Execute a bash command in a skill's environment context.
///
/// `cwd` is the effective working directory for the command (typically
/// `SKILLLITE_OUTPUT_DIR` so file outputs land there automatically).
/// `workspace` is exposed as the `SKILLLITE_WORKSPACE` env var so the
/// command can reference workspace files when needed.
///
/// The skill's `node_modules/.bin/` is still injected into PATH so CLI
/// tools (e.g. agent-browser) are found.
///
/// Returns structured text with stdout, stderr, and exit_code so the LLM
/// always sees both channels — critical for diagnosing failures.
fn execute_bash_in_skill(
    _skill_dir: &Path,
    command: &str,
    env_path: &Path,
    cwd: &Path,
    workspace: &Path,
) -> Result<String> {
    use std::process::{Command, Stdio};

    // Rewrite the command: resolve relative file-output paths to absolute paths
    // using the output directory. This is the reliable fallback because some tools
    // (e.g. agent-browser) ignore the shell's cwd and save files relative to their
    // own process directory. By injecting absolute paths, we guarantee the file
    // lands in the output directory regardless of the tool's internal behavior.
    let command = rewrite_output_paths(command, cwd);

    tracing::info!("bash_in_skill: cmd={:?} cwd={}", command, cwd.display());

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command.as_str());
    cmd.current_dir(cwd);

    // Expose workspace and output directory as env vars so LLM-generated commands
    // can use $SKILLLITE_OUTPUT_DIR/filename to produce absolute paths.
    // This is critical because some tools (e.g. agent-browser) ignore shell cwd
    // and resolve file paths relative to their own process directory.
    cmd.env("SKILLLITE_WORKSPACE", workspace.as_os_str());
    if let Some(ref output_dir) = crate::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Inject node_modules/.bin/ into PATH
    if env_path.exists() {
        let bin_dir = env_path.join("node_modules").join(".bin");
        if bin_dir.exists() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", bin_dir.display(), current_path));
        }
    }

    let output = cmd.output()
        .with_context(|| format!("Failed to execute bash command: {}", command))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    tracing::info!(
        "bash_in_skill: exit_code={} stdout_len={} stderr_len={}",
        exit_code,
        stdout.len(),
        stderr.len(),
    );

    // Always return both stdout and stderr so the LLM can see errors.
    // Format as structured text (matching execute_bash_with_env in main.rs).
    let mut result = String::new();
    let stdout_trimmed = stdout.trim();
    let stderr_trimmed = stderr.trim();

    if exit_code == 0 {
        if !stdout_trimmed.is_empty() {
            result.push_str(stdout_trimmed);
        }
        if !stderr_trimmed.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format!("[stderr]: {}", stderr_trimmed));
        }
        if result.is_empty() {
            result.push_str("Command succeeded (exit 0)");
        }
    } else {
        result.push_str(&format!("Command failed (exit {}):", exit_code));
        if !stdout_trimmed.is_empty() {
            result.push_str(&format!("\n{}", stdout_trimmed));
        }
        if !stderr_trimmed.is_empty() {
            result.push_str(&format!("\n[stderr]: {}", stderr_trimmed));
        }
    }

    Ok(result)
}

/// Rewrite relative file-output paths in a bash command to absolute paths.
///
/// Many CLI tools (e.g. `agent-browser`) do NOT save files relative to the
/// shell's current working directory.  To guarantee output files land in
/// the intended directory we resolve any "bare filename" argument that looks
/// like a file-output path (has a common file extension) into an absolute
/// path under `output_dir`.
///
/// Examples:
///   "agent-browser screenshot shot.png"
///   → "agent-browser screenshot /Users/x/.skilllite/chat/output/shot.png"
///
///   "agent-browser screenshot $SKILLLITE_OUTPUT_DIR/shot.png"
///   → unchanged (already uses env var / absolute prefix)
///
///   "agent-browser open https://example.com"
///   → unchanged (URL, not a file path)
fn rewrite_output_paths(command: &str, output_dir: &Path) -> String {
    // Common file-output extensions that should be rewritten
    const OUTPUT_EXTENSIONS: &[&str] = &[
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg",
        ".pdf", ".html", ".htm", ".json", ".csv", ".txt", ".md",
        ".webm", ".mp4",
    ];

    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.len() < 2 {
        return command.to_string();
    }

    let mut result_parts: Vec<String> = Vec::with_capacity(parts.len());
    for part in &parts {
        let lower = part.to_lowercase();

        // Skip if already absolute, uses env var, or is a URL
        let is_absolute = part.starts_with('/');
        let has_env_var = part.contains('$');
        let is_url = part.contains("://");

        let has_output_ext = OUTPUT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext));

        if has_output_ext && !is_absolute && !has_env_var && !is_url {
            // Resolve to absolute path under output_dir
            let abs = output_dir.join(part);
            result_parts.push(abs.to_string_lossy().to_string());
        } else {
            result_parts.push(part.to_string());
        }
    }

    result_parts.join(" ")
}

/// Find a loaded skill by tool name.
///
/// Supports fuzzy matching: normalizes both the query and registered names
/// so that `frontend-design` matches `frontend_design` and vice versa.
/// This is needed because LLMs sometimes use the original skill name (with hyphens)
/// instead of the sanitized tool name (with underscores).
pub fn find_skill_by_tool_name<'a>(
    skills: &'a [LoadedSkill],
    tool_name: &str,
) -> Option<&'a LoadedSkill> {
    // Exact match first (fast path)
    if let Some(skill) = skills.iter().find(|s| {
        s.tool_definitions.iter().any(|td| td.function.name == tool_name)
    }) {
        return Some(skill);
    }

    // Normalized match: replace hyphens with underscores and compare
    let normalized = sanitize_tool_name(tool_name);
    skills.iter().find(|s| {
        s.tool_definitions.iter().any(|td| td.function.name == normalized)
    })
}

/// Find a loaded skill by its original name (not tool definition name).
///
/// This is useful for finding reference-only skills that have no tool definitions
/// but are still loaded and available for documentation injection.
/// Matches both exact name and normalized name (hyphens ↔ underscores).
pub fn find_skill_by_name<'a>(
    skills: &'a [LoadedSkill],
    name: &str,
) -> Option<&'a LoadedSkill> {
    // Exact match
    if let Some(skill) = skills.iter().find(|s| s.name == name) {
        return Some(skill);
    }
    // Normalized: frontend_design matches frontend-design
    let with_hyphens = name.replace('_', "-");
    let with_underscores = name.replace('-', "_");
    skills.iter().find(|s| s.name == with_hyphens || s.name == with_underscores)
}

// ─── Phase 2.5: Security scanning integration ──────────────────────────────

/// Compute a hash of a skill's code for cache invalidation.
fn compute_skill_hash(skill_dir: &Path, metadata: &SkillMetadata) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();

    // Hash the entry point script content
    let entry_path = if !metadata.entry_point.is_empty() {
        skill_dir.join(&metadata.entry_point)
    } else {
        // Try common defaults
        let defaults = ["scripts/main.py", "main.py"];
        defaults
            .iter()
            .map(|d| skill_dir.join(d))
            .find(|p| p.exists())
            .unwrap_or_else(|| skill_dir.join("SKILL.md"))
    };

    if let Ok(content) = std::fs::read(&entry_path) {
        hasher.update(&content);
    }
    // Also include SKILL.md content
    if let Ok(skill_md) = std::fs::read(skill_dir.join("SKILL.md")) {
        hasher.update(&skill_md);
    }

    hex::encode(hasher.finalize())[..16].to_string()
}

/// Run security scan on a skill's entry point.
/// Returns formatted report string, or None if scan is clean.
fn run_security_scan(skill_dir: &Path, metadata: &SkillMetadata) -> Option<String> {
    let entry_path = if !metadata.entry_point.is_empty() {
        skill_dir.join(&metadata.entry_point)
    } else {
        let defaults = ["scripts/main.py", "main.py"];
        match defaults.iter().map(|d| skill_dir.join(d)).find(|p| p.exists()) {
            Some(p) => p,
            None => return None,
        }
    };

    if !entry_path.exists() {
        return None;
    }

    let scanner = ScriptScanner::new();
    match scanner.scan_file(&entry_path) {
        Ok(result) => {
            if result.is_safe {
                None
            } else {
                Some(crate::sandbox::security::scanner::format_scan_result_compact(&result))
            }
        }
        Err(e) => {
            tracing::warn!("Security scan failed for {}: {}", entry_path.display(), e);
            Some(format!("Security scan failed: {}. Manual review required.", e))
        }
    }
}

// ─── Phase 2.5: .skilllite.lock dependency resolution ───────────────────────
// Kept for future init_deps integration; metadata uses its own read_lock_file_packages.

#[allow(dead_code)]
/// Lock file structure for cached dependency resolution.
#[derive(Debug, serde::Deserialize)]
pub struct LockFile {
    pub compatibility_hash: String,
    pub language: String,
    pub resolved_packages: Vec<String>,
    pub resolved_at: String,
    pub resolver: String,
}

#[allow(dead_code)]
/// Read and validate a `.skilllite.lock` file for a skill.
/// Returns the resolved packages if the lock is fresh, None if stale or missing.
pub fn read_lock_file(skill_dir: &Path, compatibility: Option<&str>) -> Option<Vec<String>> {
    let lock_path = skill_dir.join(".skilllite.lock");
    if !lock_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&lock_path).ok()?;
    let lock: LockFile = serde_json::from_str(&content).ok()?;

    // Check staleness via compatibility hash
    let compat_str = compatibility.unwrap_or("");
    let current_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(compat_str.as_bytes());
        hex::encode(hasher.finalize())
    };

    if lock.compatibility_hash != current_hash {
        tracing::debug!(
            "Lock file stale for {}: hash mismatch",
            skill_dir.display()
        );
        return None;
    }

    Some(lock.resolved_packages)
}

#[allow(dead_code)]
/// Write a `.skilllite.lock` file for a skill.
pub fn write_lock_file(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    packages: &[String],
    resolver: &str,
) -> Result<()> {
    let compat_str = compatibility.unwrap_or("");
    let compat_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(compat_str.as_bytes());
        hex::encode(hasher.finalize())
    };

    let mut sorted_packages = packages.to_vec();
    sorted_packages.sort();

    let lock = serde_json::json!({
        "compatibility_hash": compat_hash,
        "language": language,
        "resolved_packages": sorted_packages,
        "resolved_at": chrono::Utc::now().to_rfc3339(),
        "resolver": resolver,
    });

    let lock_path = skill_dir.join(".skilllite.lock");
    std::fs::write(&lock_path, serde_json::to_string_pretty(&lock)? + "\n")?;

    Ok(())
}
