mod cli;
mod commands;
mod config;
mod env;
mod observability;
// mod protocol;
mod sandbox;
mod skill;

#[cfg(feature = "executor")]
mod executor;

#[cfg(feature = "agent")]
mod agent;

use anyhow::{Context, Result};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use clap::Parser;
use cli::{Cli, Commands};
use serde_json::{json, Value};

/// Guard that removes an env var on drop. Ensures no stale value between requests.
/// SAFETY: Only used in single-threaded IPC contexts (serve_stdio processes one request
/// at a time on the main thread, no tokio runtime active).
struct ScopedEnvGuard(&'static str);
impl Drop for ScopedEnvGuard {
    fn drop(&mut self) {
        // SAFETY: serve_stdio is single-threaded; no concurrent env access.
        unsafe { std::env::remove_var(self.0) };
    }
}

/// Get the allowed root directory for path validation.
fn get_allowed_root() -> Result<PathBuf> {
    let allowed_root = std::env::var("SKILLBOX_SKILLS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    allowed_root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Invalid SKILLBOX_SKILLS_ROOT: {}", e))
}

/// Validate path is within allowed root. Prevents path traversal.
fn validate_path_under_root(path: &str, path_type: &str) -> Result<PathBuf> {
    let allowed_root = get_allowed_root()?;
    let input = Path::new(path);
    let full = if input.is_absolute() {
        input.to_path_buf()
    } else {
        allowed_root.join(input)
    };
    let canonical = full
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("{} does not exist: {}", path_type, path))?;
    if !canonical.starts_with(&allowed_root) {
        anyhow::bail!("{} escapes allowed root: {}", path_type, path);
    }
    Ok(canonical)
}

/// Validate skill_dir is within allowed root. Prevents path traversal.
fn validate_skill_path(skill_dir: &str) -> Result<PathBuf> {
    validate_path_under_root(skill_dir, "Skill path")
}

fn main() -> Result<()> {
    observability::init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                serve_stdio()?;
            }
        }
        Commands::Run {
            skill_dir,
            input_json,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
        } => {
            let input_json = if input_json == "-" {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                s
            } else {
                input_json
            };
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = run_skill(&skill_dir, &input_json, allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
            println!("{}", result);
        }
        Commands::Exec {
            skill_dir,
            script_path,
            input_json,
            args,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
        } => {
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = exec_script(&skill_dir, &script_path, &input_json, args.as_ref(), allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
            println!("{}", result);
        }
        Commands::Bash {
            skill_dir,
            command,
            cache_dir,
            timeout,
            cwd,
        } => {
            let result = bash_command(&skill_dir, &command, cache_dir.as_ref(), timeout.unwrap_or(120), cwd.as_ref())?;
            println!("{}", result);
        }
        Commands::Scan {
            skill_dir,
            preview_lines,
        } => {
            let result = scan_skill(&skill_dir, preview_lines)?;
            println!("{}", result);
        }
        Commands::Validate { skill_dir } => {
            validate_skill(&skill_dir)?;
            println!("Skill validation passed!");
        }
        Commands::Info { skill_dir } => {
            show_skill_info(&skill_dir)?;
        }
        Commands::SecurityScan {
            script_path,
            allow_network,
            allow_file_ops,
            allow_process_exec,
            json,
        } => {
            security_scan_script(&script_path, allow_network, allow_file_ops, allow_process_exec, json)?;
        }
        #[cfg(feature = "agent")]
        Commands::Chat {
            api_base,
            api_key,
            model,
            workspace,
            skill_dir,
            session,
            max_iterations,
            system_prompt,
            verbose,
            message,
            plan,
            no_plan,
        } => {
            run_chat(
                api_base,
                api_key,
                model,
                workspace,
                skill_dir,
                session,
                max_iterations,
                system_prompt,
                verbose,
                message,
                plan,
                no_plan,
            )?;
        }

        // ─── Phase 3: CLI Migration Commands (flat) ─────────────────────

        Commands::Add { source, skills_dir, force, list } => {
            commands::skill::cmd_add(&source, &skills_dir, force, list)?;
        }
        Commands::Remove { skill_name, skills_dir, force } => {
            commands::skill::cmd_remove(&skill_name, &skills_dir, force)?;
        }
        Commands::List { skills_dir, json } => {
            commands::skill::cmd_list(&skills_dir, json)?;
        }
        Commands::Show { skill_name, skills_dir, json } => {
            commands::skill::cmd_show(&skill_name, &skills_dir, json)?;
        }
        Commands::InitCursor { project_dir, skills_dir, global, force } => {
            commands::ide::cmd_cursor(project_dir.as_deref(), &skills_dir, global, force)?;
        }
        Commands::InitOpencode { project_dir, skills_dir, force } => {
            commands::ide::cmd_opencode(project_dir.as_deref(), &skills_dir, force)?;
        }
        #[cfg(feature = "audit")]
        Commands::DependencyAudit { skill_dir, json } => {
            dependency_audit_skill(&skill_dir, json)?;
        }
        Commands::CleanEnv { dry_run, force } => {
            commands::env::cmd_clean(dry_run, force)?;
        }
        Commands::Reindex { skills_dir, verbose } => {
            commands::reindex::cmd_reindex(&skills_dir, verbose)?;
        }
        #[cfg(feature = "agent")]
        Commands::AgentRpc => {
            agent::rpc::serve_agent_rpc()?;
        }
    }

    Ok(())
}

/// IPC daemon: read JSON-RPC requests from stdin (one per line), write responses to stdout.
/// Request: {"jsonrpc":"2.0","id":1,"method":"run"|"exec","params":{...}}
/// Response: {"jsonrpc":"2.0","id":1,"result":{...}} or {"jsonrpc":"2.0","id":1,"error":{...}}
fn serve_stdio() -> Result<()> {
    // Suppress info logs in daemon mode (benchmark, etc.)
    // SAFETY: Called at the start of serve_stdio before any multi-threading.
    // serve_stdio is a synchronous blocking loop with no tokio runtime.
    unsafe {
        std::env::set_var("SKILLBOX_AUTO_APPROVE", "1");
        std::env::set_var("SKILLBOX_QUIET", "1");
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line.context("Failed to read stdin")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {}", e)}
                });
                writeln!(stdout, "{}", err_resp)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Object(serde_json::Map::new()));

        let result = match method {
            "run" => handle_run(&params),
            "exec" => handle_exec(&params),
            "bash" => handle_bash(&params),
            #[cfg(feature = "executor")]
            "session_create" => executor::rpc::handle_session_create(&params),
            #[cfg(feature = "executor")]
            "session_get" => executor::rpc::handle_session_get(&params),
            #[cfg(feature = "executor")]
            "session_update" => executor::rpc::handle_session_update(&params),
            #[cfg(feature = "executor")]
            "transcript_append" => executor::rpc::handle_transcript_append(&params),
            #[cfg(feature = "executor")]
            "transcript_read" => executor::rpc::handle_transcript_read(&params),
            #[cfg(feature = "executor")]
            "transcript_ensure" => executor::rpc::handle_transcript_ensure(&params),
            #[cfg(feature = "executor")]
            "memory_write" => executor::rpc::handle_memory_write(&params),
            #[cfg(feature = "executor")]
            "memory_search" => executor::rpc::handle_memory_search(&params),
            #[cfg(feature = "executor")]
            "token_count" => executor::rpc::handle_token_count(&params),
            #[cfg(feature = "executor")]
            "plan_textify" => executor::rpc::handle_plan_textify(&params),
            #[cfg(feature = "executor")]
            "plan_write" => executor::rpc::handle_plan_write(&params),
            #[cfg(feature = "executor")]
            "plan_read" => executor::rpc::handle_plan_read(&params),
            _ => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": format!("Method not found: {}", method)}
                });
                writeln!(stdout, "{}", err_resp)?;
                stdout.flush()?;
                continue;
            }
        };

        match result {
            Ok(res) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": res
                });
                writeln!(stdout, "{}", resp)?;
            }
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32603, "message": e.to_string()}
                });
                writeln!(stdout, "{}", err_resp)?;
            }
        }
        stdout.flush()?;
    }

    Ok(())
}

fn handle_run(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = sandbox::executor::ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = run_skill(skill_dir, input_json, allow_network, cache_dir_ref, limits, sandbox_level)?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_exec(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let script_path = p.get("script_path").and_then(|v| v.as_str()).context("script_path required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let args = p.get("args").and_then(|v| v.as_str()).map(|s| s.to_string());
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = sandbox::executor::ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = exec_script(
        skill_dir,
        script_path,
        input_json,
        args.as_ref(),
        allow_network,
        cache_dir_ref,
        limits,
        sandbox_level,
    )?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_bash(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let command = p.get("command").and_then(|v| v.as_str()).context("command required")?;
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout = p.get("timeout").and_then(|v| v.as_u64()).unwrap_or(120);
    let cwd = p.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());

    let output = bash_command(skill_dir, command, cache_dir.as_ref(), timeout, cwd.as_ref())?;
    Ok(serde_json::from_str(&output).unwrap_or_else(|_| json!({
        "output": output,
        "exit_code": 0
    })))
}

/// Execute a bash command for a bash-tool skill.
///
/// 1. Validates the skill is a bash-tool skill (has allowed-tools, no entry_point)
/// 2. Parses allowed patterns from SKILL.md
/// 3. Validates the command against patterns (security — Rust layer, cannot be bypassed)
/// 4. Ensures CLI dependencies are installed (npm)
/// 5. Executes the command with PATH pointing to node_modules/.bin/
fn bash_command(
    skill_dir: &str,
    command: &str,
    cache_dir: Option<&String>,
    timeout_secs: u64,
    cwd: Option<&String>,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

    // 1. Parse SKILL.md metadata
    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    // 2. Verify this is a bash-tool skill
    if !metadata.is_bash_tool_skill() {
        anyhow::bail!(
            "Skill '{}' is not a bash-tool skill (missing allowed-tools or has entry_point)",
            metadata.name
        );
    }

    // 3. Parse allowed patterns and validate command (SECURITY: Rust layer)
    let patterns = metadata.get_bash_patterns();
    if patterns.is_empty() {
        anyhow::bail!("Skill '{}' has allowed-tools but no Bash(...) patterns found", metadata.name);
    }

    sandbox::bash_validator::validate_bash_command(command, &patterns)
        .map_err(|e| anyhow::anyhow!("Command validation failed: {}", e))?;

    // 4. Ensure CLI dependencies are installed
    info_log!("[INFO] bash: ensure_environment start...");
    let env_path = env::builder::ensure_environment(
        &skill_path,
        &metadata,
        cache_dir.map(|s| s.as_str()),
    )?;
    info_log!("[INFO] bash: ensure_environment done");

    // 5. Execute command with PATH injection
    info_log!("[INFO] bash: executing command: {}", command);
    let output = execute_bash_with_env(command, &skill_path, &env_path, timeout_secs, cwd)?;

    Ok(output)
}

/// Execute a bash command in the context of a skill's environment.
///
/// - Uses `sh -c` to run the command
/// - Injects `node_modules/.bin/` into PATH so CLI tools are found
/// - Applies timeout via `wait_with_timeout()`
/// - Sets working directory to `cwd` if provided, otherwise inherits parent cwd
/// - Returns JSON with stdout, stderr, and exit_code
fn execute_bash_with_env(
    command: &str,
    _skill_dir: &std::path::Path,
    env_path: &std::path::Path,
    timeout_secs: u64,
    cwd: Option<&String>,
) -> Result<String> {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    // Set working directory so files (e.g. screenshots) are saved relative to the
    // user's workspace. In IPC mode the daemon's cwd is fixed at startup, so
    // the caller (Python SDK) passes the real workspace path via `cwd`.
    if let Some(dir) = cwd {
        let p = std::path::Path::new(dir);
        if p.is_dir() {
            cmd.current_dir(p);
        }
    }

    // Pipe stdout and stderr for capture
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Inject node_modules/.bin/ into PATH so CLI tools (e.g. agent-browser) are found
    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let bin_dir = env_path.join("node_modules").join(".bin");
        if bin_dir.exists() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", bin_dir.display(), current_path));
        }
    }

    // Spawn the process
    let mut child = cmd.spawn()
        .with_context(|| format!("Failed to spawn bash command: {}", command))?;

    // Wait with timeout and memory monitoring (reuse existing infrastructure)
    let memory_limit = sandbox::executor::ResourceLimits::from_env().max_memory_bytes();
    let (stdout, stderr, exit_code, was_killed, kill_reason) =
        sandbox::common::wait_with_timeout(&mut child, timeout_secs, memory_limit, true)?;

    if was_killed {
        if let Some(ref reason) = kill_reason {
            info_log!("[WARN] bash command killed: {}", reason);
        }
    }

    // Return structured JSON result
    let result = json!({
        "stdout": stdout.trim(),
        "stderr": stderr.trim(),
        "exit_code": exit_code,
    });

    Ok(result.to_string())
}

fn run_skill(
    skill_dir: &str,
    input_json: &str,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: sandbox::executor::ResourceLimits,
    sandbox_level: sandbox::executor::SandboxLevel,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

    // 1. Parse SKILL.md metadata
    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    // Check if this is a prompt-only skill (no entry_point)
    if metadata.entry_point.is_empty() {
        anyhow::bail!("This skill has no entry point and cannot be executed. It is a prompt-only skill.");
    }

    // 2. Validate input JSON
    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    // 3. Setup environment (venv/node_modules)
    info_log!("[INFO] ensure_environment start...");
    let env_path = env::builder::ensure_environment(&skill_path, &metadata, cache_dir.map(|s| s.as_str()))?;
    info_log!("[INFO] ensure_environment done");

    // 4. Apply CLI overrides and execute in sandbox
    let mut effective_metadata = metadata;
    
    // CLI --allow-network flag takes precedence
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    let output = sandbox::executor::run_in_sandbox_with_limits_and_level(
        &skill_path,
        &env_path,
        &effective_metadata,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

fn validate_skill(skill_dir: &str) -> Result<()> {
    let skill_path = validate_skill_path(skill_dir)?;

    // Parse and validate metadata
    let metadata = skill::metadata::parse_skill_metadata(&skill_path)?;

    // Check entry point exists (only if entry_point is specified)
    if !metadata.entry_point.is_empty() {
        let entry_path = skill_path.join(&metadata.entry_point);
        if !entry_path.exists() {
            anyhow::bail!("Entry point not found: {}", metadata.entry_point);
        }

        // Check dependencies file if language specified
        skill::deps::validate_dependencies(&skill_path, &metadata)?;
    }

    Ok(())
}

fn show_skill_info(skill_dir: &str) -> Result<()> {
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

/// Execute a specific script directly in sandbox
fn exec_script(
    skill_dir: &str,
    script_path: &str,
    input_json: &str,
    args: Option<&String>,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: sandbox::executor::ResourceLimits,
    sandbox_level: sandbox::executor::SandboxLevel,
) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;
    let full_script_path = skill_path.join(script_path);

    // Validate script exists
    if !full_script_path.exists() {
        anyhow::bail!("Script not found: {}", full_script_path.display());
    }

    // Prevent script_path from escaping skill_dir (e.g. ../../../etc/passwd)
    let full_canonical = full_script_path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("Script path does not exist: {}", script_path))?;
    if !full_canonical.starts_with(&skill_path) {
        anyhow::bail!("Script path escapes skill directory: {}", script_path);
    }

    // Detect language from script extension
    let language = detect_script_language(&full_script_path)?;

    // Validate input JSON
    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    // Try to parse SKILL.md for network policy and dependencies (optional)
    let (metadata, env_path) = if skill_path.join("SKILL.md").exists() {
        let mut meta = skill::metadata::parse_skill_metadata(&skill_path)?;
        // Override entry_point with the specified script
        meta.entry_point = script_path.to_string();
        meta.language = Some(language.clone());
        
        // Setup environment based on skill dependencies
        let env = env::builder::ensure_environment(&skill_path, &meta, cache_dir.map(|s| s.as_str()))?;
        (meta, env)
    } else {
        // No SKILL.md, create minimal metadata
        let meta = skill::metadata::SkillMetadata {
            name: skill_path.file_name()
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
        };
        (meta, std::path::PathBuf::new())
    };

    // Apply network settings
    let mut effective_metadata = metadata;
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    // Pass script args via env for executor. Use guard to clear on drop so no stale
    // value leaks to subsequent requests (IPC mode processes one request at a time).
    // SAFETY: exec_script is called from single-threaded contexts only (CLI or
    // serve_stdio IPC loop). No concurrent threads access env vars.
    let _args_guard = if let Some(ref args_str) = args {
        unsafe { std::env::set_var("SKILLBOX_SCRIPT_ARGS", args_str) };
        Some(ScopedEnvGuard("SKILLBOX_SCRIPT_ARGS"))
    } else {
        unsafe { std::env::remove_var("SKILLBOX_SCRIPT_ARGS") };
        None
    };

    // Execute in sandbox
    let output = sandbox::executor::run_in_sandbox_with_limits_and_level(
        &skill_path,
        &env_path,
        &effective_metadata,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

/// Detect script language from file extension
fn detect_script_language(script_path: &std::path::Path) -> Result<String> {
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
            // Check shebang for scripts without extension
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

/// Perform security scan on a script
fn security_scan_script(
    script_path: &str,
    allow_network: bool,
    allow_file_ops: bool,
    allow_process_exec: bool,
    json_output: bool,
) -> Result<()> {
    use crate::sandbox::security::{format_scan_result, format_scan_result_json, ScriptScanner};

    // Validate script path is within allowed root (prevents path traversal)
    let path = validate_path_under_root(script_path, "Script path")?;

    // Create scanner with specified permissions
    let scanner = ScriptScanner::new()
        .allow_network(allow_network)
        .allow_file_ops(allow_file_ops)
        .allow_process_exec(allow_process_exec);

    // Scan the script
    let scan_result = scanner.scan_file(&path)?;

    // Display results
    if json_output {
        println!("{}", format_scan_result_json(&scan_result));
    } else {
        println!("Security Scan Results for: {}\n", path.display());
        println!("{}", format_scan_result(&scan_result));
    }

    Ok(())
}

/// Audit skill dependencies for known vulnerabilities via OSV.dev
#[cfg(feature = "audit")]
fn dependency_audit_skill(skill_dir: &str, json_output: bool) -> Result<()> {
    use crate::sandbox::security::dependency_audit;

    let path = validate_path_under_root(skill_dir, "Skill directory")?;
    let result = dependency_audit::audit_skill_dependencies(&path)?;

    if json_output {
        println!("{}", dependency_audit::format_audit_result_json(&result));
    } else {
        println!("{}", dependency_audit::format_audit_result(&result));
    }

    // Exit with code 1 if vulnerabilities found (useful for CI)
    if result.vulnerable_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}

/// Scan skill directory and return JSON with all executable scripts
fn scan_skill(skill_dir: &str, preview_lines: usize) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

    let mut result = serde_json::json!({
        "skill_dir": skill_dir,
        "has_skill_md": false,
        "skill_metadata": null,
        "scripts": [],
        "directories": {
            "scripts": false,
            "references": false,
            "assets": false
        }
    });

    // Check for SKILL.md and parse metadata
    let skill_md_path = skill_path.join("SKILL.md");
    if skill_md_path.exists() {
        result["has_skill_md"] = serde_json::json!(true);
        if let Ok(metadata) = skill::metadata::parse_skill_metadata(&skill_path) {
            result["skill_metadata"] = serde_json::json!({
                "name": metadata.name,
                "description": metadata.description,
                "entry_point": if metadata.entry_point.is_empty() { None } else { Some(&metadata.entry_point) },
                "language": metadata.language,
                "network_enabled": metadata.network.enabled,
                "compatibility": metadata.compatibility
            });
        }
    }

    // Check standard directories
    result["directories"]["scripts"] = serde_json::json!(skill_path.join("scripts").exists());
    result["directories"]["references"] = serde_json::json!(skill_path.join("references").exists());
    result["directories"]["assets"] = serde_json::json!(skill_path.join("assets").exists());

    // Scan for executable scripts
    let mut scripts = Vec::new();
    scan_scripts_recursive(&skill_path, &skill_path, &mut scripts, preview_lines)?;

    result["scripts"] = serde_json::json!(scripts);

    serde_json::to_string_pretty(&result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize scan result: {}", e))
}

/// Recursively scan for executable scripts
fn scan_scripts_recursive(
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    scripts: &mut Vec<serde_json::Value>,
    preview_lines: usize,
) -> Result<()> {
    use std::fs;

    let entries = fs::read_dir(current_path)
        .map_err(|e| anyhow::anyhow!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and directories
        if file_name.starts_with('.') {
            continue;
        }

        // Skip common non-script directories
        if path.is_dir() {
            let skip_dirs = ["node_modules", "__pycache__", ".git", "venv", ".venv", "assets", "references"];
            if skip_dirs.contains(&file_name.as_str()) {
                continue;
            }
            scan_scripts_recursive(base_path, &path, scripts, preview_lines)?;
            continue;
        }

        // Check if it's an executable script
        if let Some(script_info) = analyze_script_file(&path, base_path, preview_lines) {
            scripts.push(script_info);
        }
    }

    Ok(())
}

/// Analyze a single script file and return its metadata
fn analyze_script_file(
    file_path: &std::path::Path,
    base_path: &std::path::Path,
    preview_lines: usize,
) -> Option<serde_json::Value> {
    use std::fs;

    let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    
    // Supported script extensions
    let (language, is_script) = match extension {
        "py" => ("python", true),
        "js" | "mjs" | "cjs" => ("node", true),
        "ts" => ("typescript", true),
        "sh" | "bash" => ("shell", true),
        "" => {
            // Check shebang for files without extension
            if let Ok(content) = fs::read_to_string(file_path) {
                if let Some(first_line) = content.lines().next() {
                    if first_line.starts_with("#!") {
                        if first_line.contains("python") {
                            ("python", true)
                        } else if first_line.contains("node") {
                            ("node", true)
                        } else if first_line.contains("bash") || first_line.contains("sh") {
                            ("shell", true)
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };

    if !is_script {
        return None;
    }

    let relative_path = file_path.strip_prefix(base_path).ok()?;
    let content = fs::read_to_string(file_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Extract preview (first N lines)
    let preview: String = lines.iter()
        .take(preview_lines)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    // Extract docstring/description
    let description = extract_script_description(&content, language);

    // Detect if script has main entry point
    let has_main = detect_main_entry(&content, language);

    // Detect CLI arguments usage
    let uses_argparse = detect_argparse_usage(&content, language);

    // Detect stdin/stdout usage
    let uses_stdio = detect_stdio_usage(&content, language);

    Some(serde_json::json!({
        "path": relative_path.to_string_lossy(),
        "language": language,
        "total_lines": total_lines,
        "preview": preview,
        "description": description,
        "has_main_entry": has_main,
        "uses_argparse": uses_argparse,
        "uses_stdio": uses_stdio,
        "file_size_bytes": fs::metadata(file_path).map(|m| m.len()).unwrap_or(0)
    }))
}

/// Extract script description from docstring or comments
fn extract_script_description(content: &str, language: &str) -> Option<String> {
    match language {
        "python" => {
            // Look for module docstring
            let trimmed = content.trim_start();
            if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                let quote = if trimmed.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };
                if let Some(start) = trimmed.find(quote) {
                    let rest = &trimmed[start + 3..];
                    if let Some(end) = rest.find(quote) {
                        return Some(rest[..end].trim().to_string());
                    }
                }
            }
            // Look for leading comments
            let mut desc_lines = Vec::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
                    desc_lines.push(trimmed.trim_start_matches('#').trim());
                } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        "node" | "typescript" => {
            // Look for JSDoc or leading comments
            let trimmed = content.trim_start();
            if trimmed.starts_with("/**") {
                if let Some(end) = trimmed.find("*/") {
                    let doc = &trimmed[3..end];
                    let cleaned: Vec<&str> = doc.lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty())
                        .collect();
                    return Some(cleaned.join(" "));
                }
            }
            // Look for // comments
            let mut desc_lines = Vec::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") {
                    desc_lines.push(trimmed.trim_start_matches('/').trim());
                } else if !trimmed.is_empty() {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        "shell" => {
            // Look for leading comments (skip shebang)
            let mut desc_lines = Vec::new();
            let mut skip_shebang = true;
            for line in content.lines() {
                let trimmed = line.trim();
                if skip_shebang && trimmed.starts_with("#!") {
                    skip_shebang = false;
                    continue;
                }
                if trimmed.starts_with('#') {
                    desc_lines.push(trimmed.trim_start_matches('#').trim());
                } else if !trimmed.is_empty() {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        _ => None,
    }
}

/// Detect if script has a main entry point
fn detect_main_entry(content: &str, language: &str) -> bool {
    match language {
        "python" => content.contains("if __name__") && content.contains("__main__"),
        "node" | "typescript" => {
            content.contains("require.main === module") || 
            content.contains("import.meta.main") ||
            // Check for top-level execution patterns
            (!content.contains("module.exports") && !content.contains("export "))
        }
        "shell" => true, // Shell scripts are always executable
        _ => false,
    }
}

/// Detect if script uses argument parsing
fn detect_argparse_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("argparse") || 
            content.contains("sys.argv") || 
            content.contains("click") ||
            content.contains("typer")
        }
        "node" | "typescript" => {
            content.contains("process.argv") || 
            content.contains("yargs") || 
            content.contains("commander") ||
            content.contains("minimist")
        }
        "shell" => {
            content.contains("$1") || 
            content.contains("$@") || 
            content.contains("getopts") ||
            content.contains("${1")
        }
        _ => false,
    }
}

/// Detect if script uses stdin/stdout for I/O
fn detect_stdio_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("sys.stdin") || 
            content.contains("input()") || 
            content.contains("json.load(sys.stdin)") ||
            content.contains("print(") ||
            content.contains("json.dumps")
        }
        "node" | "typescript" => {
            content.contains("process.stdin") || 
            content.contains("readline") ||
            content.contains("console.log") ||
            content.contains("JSON.stringify")
        }
        "shell" => {
            content.contains("read ") || 
            content.contains("echo ") ||
            content.contains("cat ")
        }
        _ => false,
    }
}

// ─── Agent Chat (feature = "agent") ─────────────────────────────────────────

#[cfg(feature = "agent")]
fn run_chat(
    api_base: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    workspace: Option<String>,
    skill_dirs: Vec<String>,
    session_key: String,
    max_iterations: usize,
    system_prompt: Option<String>,
    verbose: bool,
    single_message: Option<String>,
    plan: bool,
    no_plan: bool,
) -> Result<()> {
    use agent::types::*;
    use agent::{chat_session::ChatSession, skills};

    // Build config from CLI args + env
    let mut config = AgentConfig::from_env();
    if let Some(base) = api_base {
        config.api_base = base;
    }
    if let Some(key) = api_key {
        config.api_key = key;
    }
    if let Some(m) = model {
        config.model = m;
    }
    if let Some(ws) = workspace {
        config.workspace = ws;
    }
    config.max_iterations = max_iterations;
    config.system_prompt = system_prompt;
    config.verbose = verbose;

    // Set default output directory to ~/.skilllite/chat/output/ (matching Python SDK)
    // Only if SKILLLITE_OUTPUT_DIR is not already set by user
    // SAFETY: Called before tokio::runtime::Runtime::new() below, so no other
    // threads exist yet. All env var mutations happen in the single main thread.
    if std::env::var("SKILLLITE_OUTPUT_DIR").is_err() {
        let chat_output = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output");
        unsafe { std::env::set_var("SKILLLITE_OUTPUT_DIR", chat_output.to_string_lossy().as_ref()) };
    }

    // Ensure output directory exists so skills can write files there immediately
    if let Ok(output_dir) = std::env::var("SKILLLITE_OUTPUT_DIR") {
        let p = PathBuf::from(&output_dir);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }

    // Enable task planning: --plan > --no-plan > env var > default (true)
    config.enable_task_planning = if plan {
        true
    } else if no_plan {
        false
    } else {
        std::env::var("SKILLLITE_ENABLE_TASK_PLANNING")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(true) // Default to true when skills are available
    };

    // Validate API key
    if config.api_key.is_empty() {
        anyhow::bail!(
            "API key required. Set OPENAI_API_KEY env var or use --api-key flag."
        );
    }

    // Auto-discover skill directories if none specified
    let effective_skill_dirs = if skill_dirs.is_empty() {
        let ws = Path::new(&config.workspace);
        let mut auto_dirs = Vec::new();
        // Check common skill directory names
        for name in &[".skills", "skills"] {
            let dir = ws.join(name);
            if dir.is_dir() {
                // Each subdirectory is a skill
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join("SKILL.md").exists() {
                            auto_dirs.push(path.to_string_lossy().to_string());
                        }
                    }
                }
                if auto_dirs.is_empty() {
                    // Might be a flat structure — add the directory itself
                    auto_dirs.push(dir.to_string_lossy().to_string());
                }
            }
        }
        if !auto_dirs.is_empty() {
            eprintln!("🔍 Auto-discovered {} skill(s) in workspace", auto_dirs.len());
        }
        auto_dirs
    } else {
        skill_dirs
    };

    // Load skills
    let loaded_skills = skills::load_skills(&effective_skill_dirs);
    if !loaded_skills.is_empty() {
        eprintln!("📦 Loaded {} skill(s):", loaded_skills.len());
        for s in &loaded_skills {
            eprintln!("   - {}", s.name);
        }
    }

    // Build tokio runtime
    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    if let Some(msg) = single_message {
        // Single-shot mode
        rt.block_on(async {
            let mut session = ChatSession::new(config, &session_key, loaded_skills);
            let mut sink = TerminalEventSink::new(verbose);
            let response = session.run_turn(&msg, &mut sink).await?;
            println!("\n{}", response);
            Ok(())
        })
    } else {
        // Interactive REPL mode
        rt.block_on(async {
            run_interactive_chat(config, &session_key, loaded_skills, verbose).await
        })
    }
}

#[cfg(feature = "agent")]
async fn run_interactive_chat(
    config: agent::types::AgentConfig,
    session_key: &str,
    skills: Vec<agent::skills::LoadedSkill>,
    verbose: bool,
) -> Result<()> {
    use agent::types::*;
    use agent::chat_session::ChatSession;

    eprintln!("🤖 SkillBox Chat (model: {})", config.model);
    eprintln!("   Type /exit to quit, /clear to reset, /compact to compress history\n");

    let mut session = ChatSession::new(config, session_key, skills);
    let mut sink = TerminalEventSink::new(verbose);

    // Setup rustyline for readline-like input
    let mut rl = rustyline::DefaultEditor::new()
        .map_err(|e| anyhow::anyhow!("Failed to create line editor: {}", e))?;

    loop {
        let readline = rl.readline("You> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(input);

                // Handle slash commands
                match input {
                    "/exit" | "/quit" | "/q" => {
                        eprintln!("👋 Bye!");
                        break;
                    }
                    "/clear" => {
                        session.clear().await?;
                        eprintln!("🗑️  Session cleared.");
                        continue;
                    }
                    "/compact" => {
                        eprintln!("📦 Compacting history...");
                        // Compaction happens automatically in run_turn
                        continue;
                    }
                    _ => {}
                }

                // Run the turn
                eprint!("\nAssistant> ");
                match session.run_turn(input, &mut sink).await {
                    Ok(_response) => {
                        eprintln!(); // newline after streaming
                    }
                    Err(e) => {
                        eprintln!("\n❌ Error: {}", e);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                eprintln!("👋 Bye!");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
