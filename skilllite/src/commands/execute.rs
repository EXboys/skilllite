//! Core execution commands: run, exec, bash.
//!
//! Implements run_skill, exec_script, bash_command, validate_skill, show_skill_info.

use skilllite_core::path_validation::validate_skill_path;
use skilllite_sandbox::runner::SandboxConfig;
use skilllite_core::skill;
use anyhow::{Context, Result};
use serde_json::json;
use std::path::Path;
use std::sync::Mutex;

/// Mutex for exec_script: it uses process-global SKILLBOX_SCRIPT_ARGS env var,
/// so concurrent exec calls must be serialized. run and bash do not need this.
static EXEC_ENV_MUTEX: Mutex<()> = Mutex::new(());

use skilllite_core::config::ScopedEnvGuard;

/// Run a skill with the given input (requires entry_point in SKILL.md).
pub fn run_skill(
    skill_dir: &str,
    input_json: &str,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: skilllite_sandbox::runner::ResourceLimits,
    sandbox_level: skilllite_sandbox::runner::SandboxLevel,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    if metadata.entry_point.is_empty() {
        anyhow::bail!("This skill has no entry point and cannot be executed. It is a prompt-only skill.");
    }

    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    skilllite_sandbox::info_log!("[INFO] ensure_environment start...");
    let env_path = skilllite_sandbox::env::builder::ensure_environment(&skill_path, &metadata, cache_dir.map(|s| s.as_str()))?;
    skilllite_sandbox::info_log!("[INFO] ensure_environment done");

    let mut effective_metadata = metadata;
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    let runtime = skilllite_sandbox::env::builder::build_runtime_paths(&env_path);
    let config = build_sandbox_config(&skill_path, &effective_metadata);
    let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level(
        &skill_path,
        &runtime,
        &config,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

/// Execute a specific script directly in sandbox.
pub fn exec_script(
    skill_dir: &str,
    script_path: &str,
    input_json: &str,
    args: Option<&String>,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: skilllite_sandbox::runner::ResourceLimits,
    sandbox_level: skilllite_sandbox::runner::SandboxLevel,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;
    let full_script_path = skill_path.join(script_path);

    if !full_script_path.exists() {
        anyhow::bail!("Script not found: {}", full_script_path.display());
    }

    let full_canonical = full_script_path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Script path does not exist: {}", script_path))?;
    if !full_canonical.starts_with(&skill_path) {
        anyhow::bail!("Script path escapes skill directory: {}", script_path);
    }

    let language = detect_script_language(&full_script_path)?;

    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    let (metadata, env_path) = if skill_path.join("SKILL.md").exists() {
        let mut meta = skill::metadata::parse_skill_metadata(&skill_path)?;
        meta.entry_point = script_path.to_string();
        meta.language = Some(language.clone());
        let env = skilllite_sandbox::env::builder::ensure_environment(&skill_path, &meta, cache_dir.map(|s| s.as_str()))?;
        (meta, env)
    } else {
        let meta = skill::metadata::SkillMetadata {
            name: skill_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            entry_point: script_path.to_string(),
            language: Some(language.clone()),
            description: None,
            compatibility: None,
            network: skill::metadata::NetworkPolicy::default(),
            resolved_packages: None,
            allowed_tools: None,
            requires_elevated_permissions: false,
        };
        (meta, std::path::PathBuf::new())
    };

    let mut effective_metadata = metadata;
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    let _guard = EXEC_ENV_MUTEX.lock().map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
    let _args_guard = if let Some(ref args_str) = args {
        skilllite_core::config::set_env_var("SKILLBOX_SCRIPT_ARGS", args_str);
        Some(ScopedEnvGuard("SKILLBOX_SCRIPT_ARGS"))
    } else {
        skilllite_core::config::remove_env_var("SKILLBOX_SCRIPT_ARGS");
        None
    };

    let runtime = skilllite_sandbox::env::builder::build_runtime_paths(&env_path);
    let config = build_sandbox_config(&skill_path, &effective_metadata);
    let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level(
        &skill_path,
        &runtime,
        &config,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

/// Execute a bash command for a bash-tool skill.
pub fn bash_command(
    skill_dir: &str,
    command: &str,
    cache_dir: Option<&String>,
    timeout_secs: u64,
    cwd: Option<&String>,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    if !metadata.is_bash_tool_skill() {
        anyhow::bail!(
            "Skill '{}' is not a bash-tool skill (missing allowed-tools or has entry_point)",
            metadata.name
        );
    }

    let skill_patterns = metadata.get_bash_patterns();
    if skill_patterns.is_empty() {
        anyhow::bail!("Skill '{}' has allowed-tools but no Bash(...) patterns found", metadata.name);
    }

    let validator_patterns: Vec<skilllite_sandbox::bash_validator::BashToolPattern> = skill_patterns
        .into_iter()
        .map(|p| skilllite_sandbox::bash_validator::BashToolPattern {
            command_prefix: p.command_prefix,
            raw_pattern: p.raw_pattern,
        })
        .collect();
    skilllite_sandbox::bash_validator::validate_bash_command(command, &validator_patterns)
        .map_err(|e| anyhow::anyhow!("Command validation failed: {}", e))?;

    skilllite_sandbox::info_log!("[INFO] bash: ensure_environment start...");
    let env_path = skilllite_sandbox::env::builder::ensure_environment(
        &skill_path,
        &metadata,
        cache_dir.map(|s| s.as_str()),
    )?;
    skilllite_sandbox::info_log!("[INFO] bash: ensure_environment done");

    skilllite_sandbox::info_log!("[INFO] bash: executing command: {}", command);
    let output = execute_bash_with_env(command, &skill_path, &env_path, timeout_secs, cwd)?;

    Ok(output)
}

fn execute_bash_with_env(
    command: &str,
    _skill_dir: &Path,
    env_path: &Path,
    timeout_secs: u64,
    cwd: Option<&String>,
) -> Result<String> {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    if let Some(dir) = cwd {
        let p = Path::new(dir);
        if p.is_dir() {
            cmd.current_dir(p);
        }
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let bin_dir = env_path.join("node_modules").join(".bin");
        if bin_dir.exists() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", bin_dir.display(), current_path));
        }
    }

    let mut child = cmd.spawn()
        .with_context(|| format!("Failed to spawn bash command: {}", command))?;

    let memory_limit = skilllite_sandbox::runner::ResourceLimits::from_env().max_memory_bytes();
    let (stdout, stderr, exit_code, was_killed, kill_reason) =
        skilllite_sandbox::common::wait_with_timeout(&mut child, timeout_secs, memory_limit, true)?;

    if was_killed {
        if let Some(ref reason) = kill_reason {
            skilllite_sandbox::info_log!("[WARN] bash command killed: {}", reason);
        }
    }

    let result = json!({
        "stdout": stdout.trim(),
        "stderr": stderr.trim(),
        "exit_code": exit_code,
    });

    Ok(result.to_string())
}

/// Validate a skill without running it.
pub fn validate_skill(skill_dir: &str) -> Result<()> {
    let skill_path = validate_skill_path(skill_dir)?;
    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    if !metadata.entry_point.is_empty() {
        let entry_path = skill_path.join(&metadata.entry_point);
        if !entry_path.exists() {
            anyhow::bail!("Entry point not found: {}", metadata.entry_point);
        }
        skill::deps::validate_dependencies(&skill_path, &metadata)?;
    }

    Ok(())
}

/// Show skill information.
pub fn show_skill_info(skill_dir: &str) -> Result<()> {
    let skill_path = validate_skill_path(skill_dir)?;
    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    println!("Skill Information:");
    println!("  Name: {}", metadata.name);
    if metadata.entry_point.is_empty() {
        println!("  Entry Point: (none - prompt-only skill)");
    } else {
        println!("  Entry Point: {}", metadata.entry_point);
    }
    println!(
        "  Language: {}",
        metadata.language.as_deref().unwrap_or("auto-detect")
    );
    println!("  Network Enabled: {}", metadata.network.enabled);
    if !metadata.network.outbound.is_empty() {
        println!("  Outbound Whitelist:");
        for host in &metadata.network.outbound {
            println!("    - {}", host);
        }
    }

    Ok(())
}

/// Build a `SandboxConfig` from `SkillMetadata`, resolving language via `detect_language`.
fn build_sandbox_config(skill_dir: &Path, metadata: &skill::metadata::SkillMetadata) -> SandboxConfig {
    SandboxConfig {
        name: metadata.name.clone(),
        entry_point: metadata.entry_point.clone(),
        language: skill::metadata::detect_language(skill_dir, metadata),
        network_enabled: metadata.network.enabled,
        network_outbound: metadata.network.outbound.clone(),
        uses_playwright: metadata.uses_playwright(),
    }
}

/// Detect script language from file extension.
fn detect_script_language(script_path: &Path) -> Result<String> {
    let extension = script_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match extension {
        "py" => Ok("python".to_string()),
        "js" | "mjs" | "cjs" => Ok("node".to_string()),
        "ts" => Ok("node".to_string()),
        "sh" | "bash" => Ok("shell".to_string()),
        "" => {
            if let Ok(content) = std::fs::read_to_string(script_path) {
                if let Some(first_line) = content.lines().next() {
                    if first_line.starts_with("#!") {
                        if first_line.contains("python") {
                            return Ok("python".to_string());
                        } else if first_line.contains("node") {
                            return Ok("node".to_string());
                        } else if first_line.contains("bash") || first_line.contains("sh") {
                            return Ok("shell".to_string());
                        }
                    }
                }
            }
            anyhow::bail!("Cannot detect language for script: {}", script_path.display())
        }
        _ => anyhow::bail!("Unsupported script extension: .{}", extension),
    }
}
