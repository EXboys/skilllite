//! Skill execution: dispatches tool calls to sandbox runner.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use skilllite_sandbox::runner::{ResourceLimits, SandboxConfig, SandboxLevel};
use skilllite_core::skill::metadata::{self, SkillMetadata};

use crate::types::{EventSink, ToolResult};

use super::LoadedSkill;
use super::loader::sanitize_tool_name;
use super::security::{compute_skill_hash, run_security_scan};

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
    let cache_dir = skilllite_core::config::CacheConfig::cache_dir();
    let env_path = skilllite_sandbox::env::builder::ensure_environment(
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
        let validator_patterns: Vec<skilllite_sandbox::bash_validator::BashToolPattern> = skill_patterns
            .into_iter()
            .map(|p| skilllite_sandbox::bash_validator::BashToolPattern {
                command_prefix: p.command_prefix,
                raw_pattern: p.raw_pattern,
            })
            .collect();
        skilllite_sandbox::bash_validator::validate_bash_command(command, &validator_patterns)
            .map_err(|e| anyhow::anyhow!("Command validation failed: {}", e))?;

        // Execute bash command (same logic as main.rs bash_command).
        // Resolve the effective cwd: prefer SKILLLITE_OUTPUT_DIR so file outputs
        // (screenshots, PDFs, etc.) land in the output directory automatically.
        let effective_cwd = skilllite_core::config::PathsConfig::from_env()
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

        let runtime = skilllite_sandbox::env::builder::build_runtime_paths(&env_path);
        let config = build_sandbox_config(skill_dir, &effective_metadata);
        let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level(
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
    skill_dir: &Path,
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
    if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Inject node_modules/.bin/ into PATH (from env cache or from skill_dir)
    let bin_dir = if env_path.exists() {
        env_path.join("node_modules").join(".bin")
    } else {
        skill_dir.join("node_modules").join(".bin")
    };
    if bin_dir.exists() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{}", bin_dir.display(), current_path));
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

