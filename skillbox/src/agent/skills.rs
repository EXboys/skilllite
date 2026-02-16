//! Skills invocation: wraps sandbox run/exec for the agent layer.
//!
//! Since Agent is in the same process as Sandbox, we call the sandbox
//! executor directly (no IPC needed). Ported from Python `ToolCallHandler`.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::sandbox::executor::{ResourceLimits, SandboxLevel};
use crate::skill::metadata::{self, SkillMetadata};

use super::types::{EventSink, ToolDefinition, FunctionDef, ToolResult};

/// A loaded skill ready for invocation.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub name: String,
    pub skill_dir: PathBuf,
    pub metadata: SkillMetadata,
    pub tool_definitions: Vec<ToolDefinition>,
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
    let tool_defs = if metadata.is_bash_tool_skill() {
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
        // Regular skill with entry point: flexible input
        let desc = metadata.description.clone().unwrap_or_else(|| {
            format!("Execute skill: {}", name)
        });
        vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: tool_name,
                description: desc,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": true
                }),
            },
        }]
    } else {
        // Prompt-only skill: no tool definition
        Vec::new()
    };

    Some(LoadedSkill {
        name,
        skill_dir: skill_dir.to_path_buf(),
        metadata,
        tool_definitions: tool_defs,
    })
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

fn execute_skill_inner(
    skill: &LoadedSkill,
    _tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> Result<String> {
    let skill_dir = &skill.skill_dir;
    let metadata = &skill.metadata;

    // For Level 3, check with user before executing
    let sandbox_level = SandboxLevel::from_env_or_cli(None);
    if sandbox_level == SandboxLevel::Level3 {
        let prompt = format!(
            "Skill '{}' wants to execute code (sandbox level 3). Allow?",
            skill.name
        );
        if !event_sink.on_confirmation_request(&prompt) {
            return Ok("Execution cancelled by user.".to_string());
        }
    }

    // Setup environment
    let cache_dir: Option<String> = std::env::var("SKILLBOX_CACHE_DIR").ok();
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

        // Validate against allowed patterns
        let patterns = metadata.get_bash_patterns();
        crate::sandbox::bash_validator::validate_bash_command(command, &patterns)
            .map_err(|e| anyhow::anyhow!("Command validation failed: {}", e))?;

        // Execute bash command (same logic as main.rs bash_command).
        // Resolve the effective cwd: prefer SKILLLITE_OUTPUT_DIR so file outputs
        // (screenshots, PDFs, etc.) land in the output directory automatically,
        // even when the LLM uses a relative filename like "screenshot.png".
        let effective_cwd = std::env::var("SKILLLITE_OUTPUT_DIR")
            .ok()
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| workspace.to_path_buf());

        execute_bash_in_skill(skill_dir, command, &env_path, &effective_cwd, workspace)
    } else {
        // Regular skill: pass arguments as input JSON
        let input_json = if arguments.trim().is_empty() || arguments.trim() == "{}" {
            "{}".to_string()
        } else {
            arguments.to_string()
        };

        // Validate input JSON
        let _: Value = serde_json::from_str(&input_json)
            .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

        // Execute in sandbox (same process, direct call)
        let output = crate::sandbox::executor::run_in_sandbox_with_limits_and_level(
            skill_dir,
            &env_path,
            metadata,
            &input_json,
            limits,
            sandbox_level,
        )?;

        Ok(output)
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
    if let Ok(output_dir) = std::env::var("SKILLLITE_OUTPUT_DIR") {
        cmd.env("SKILLLITE_OUTPUT_DIR", &output_dir);
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
pub fn find_skill_by_tool_name<'a>(
    skills: &'a [LoadedSkill],
    tool_name: &str,
) -> Option<&'a LoadedSkill> {
    skills.iter().find(|s| {
        s.tool_definitions.iter().any(|td| td.function.name == tool_name)
    })
}
