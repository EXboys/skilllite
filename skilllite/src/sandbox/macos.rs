#![cfg(target_os = "macos")]

use crate::sandbox::common::{
    wait_with_timeout,
    DEFAULT_FILE_SIZE_LIMIT_MB,
    DEFAULT_MAX_PROCESSES,
};
use crate::sandbox::executor::{ExecutionResult, RuntimePaths, SandboxConfig};
use crate::sandbox::move_protection::{
    generate_log_tag, generate_move_blocking_rules, get_session_suffix,
};
use crate::sandbox::network_proxy::{ProxyConfig, ProxyManager};
use crate::sandbox::security::policy::{self as security_policy, HomePathStyle, ResolvedNetworkPolicy};
use crate::sandbox::seatbelt::generate_seatbelt_mandatory_deny_patterns;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Execute a skill in a macOS sandbox with custom resource limits
pub fn execute_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::sandbox::executor::ResourceLimits,
) -> Result<ExecutionResult> {
    if std::env::var("SKILLBOX_NO_SANDBOX").is_ok() {
        eprintln!("[WARN] Sandbox disabled via SKILLBOX_NO_SANDBOX - running without protection");
        crate::info_log!("[INFO] using simple execution (no sandbox-exec)");
        return execute_simple_with_limits(skill_dir, runtime, config, input_json, limits);
    }

    if security_policy::should_allow_playwright() && config.uses_playwright {
        crate::info_log!(
            "[INFO] Skill {} uses Playwright; skipping sandbox (SKILLBOX_ALLOW_PLAYWRIGHT/L2)",
            config.name
        );
        return execute_simple_with_limits(skill_dir, runtime, config, input_json, limits);
    }
    
    crate::info_log!("[INFO] using sandbox-exec (Seatbelt)...");
    match execute_with_sandbox(skill_dir, runtime, config, input_json, limits) {
        Ok(result) if result.exit_code == 0 => Ok(result),
        Ok(_) | Err(_) => {
            eprintln!("[WARN] Sandbox execution failed, falling back to simple execution");
            crate::observability::security_sandbox_fallback(
                &config.name,
                "seatbelt_exec_failed",
            );
            execute_simple_with_limits(skill_dir, runtime, config, input_json, limits)
        }
    }
}

/// Simple execution without sandbox (fallback for when sandbox-exec is unavailable)
pub fn execute_simple_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::sandbox::executor::ResourceLimits,
) -> Result<ExecutionResult> {
    crate::info_log!("[INFO] simple: executing {}...", config.entry_point);
    let language = &config.language;
    
    let entry_point = &config.entry_point;

    // Create temporary directory for work
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    // Prepare command based on language
    let mut cmd = match language.as_str() {
        "python" => {
            let mut c = Command::new(&runtime.python);
            c.arg(entry_point);
            c
        }
        "node" => {
            let mut c = Command::new(&runtime.node);
            c.arg(entry_point);

            if let Some(ref node_modules) = runtime.node_modules {
                c.env("NODE_PATH", node_modules);
            }
            c
        }
        _ => {
            anyhow::bail!("Unsupported language: {}", language);
        }
    };

    // Add script arguments from SKILLBOX_SCRIPT_ARGS environment variable
    // This allows passing CLI arguments to scripts that use argparse
    if let Ok(script_args) = std::env::var("SKILLBOX_SCRIPT_ARGS") {
        if !script_args.is_empty() {
            for arg in script_args.split_whitespace() {
                cmd.arg(arg);
            }
        }
    }

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set environment variables
    cmd.env("SKILLBOX_SANDBOX", "0");
    cmd.env("TMPDIR", work_dir);

    if !config.network_enabled {
        cmd.env("SKILLBOX_NETWORK_DISABLED", "1");
    }

    // Spawn the process
    crate::info_log!("[INFO] simple: spawning skill process...");
    let mut child = cmd.spawn().with_context(|| "Failed to spawn skill process")?;
    crate::info_log!("[INFO] simple: writing stdin...");

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }
    crate::info_log!("[INFO] simple: waiting for child (timeout {}s)...", limits.timeout_secs);

    // Wait with resource limits
    let memory_limit_bytes = limits.max_memory_bytes();
    let (stdout, stderr, exit_code, _, _) = wait_with_timeout(
        &mut child,
        limits.timeout_secs,
        memory_limit_bytes,
        true,  // stream stderr so user sees Pillow/playwright progress messages
    )?;

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Execute with macOS sandbox-exec with resource limits and network proxy (pure Rust, no script injection)
fn execute_with_sandbox(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::sandbox::executor::ResourceLimits,
) -> Result<ExecutionResult> {
    use std::os::unix::process::CommandExt;
    
    let language = &config.language;
    let entry_point = &config.entry_point;

    // Create temporary directory for sandbox profile and work
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    let network_policy = security_policy::resolve_network_policy(
        config.network_enabled,
        &config.network_outbound,
    );

    // Start network proxy when policy requires domain filtering
    let proxy_manager = if security_policy::should_use_proxy(&network_policy) {
        let domains = match &network_policy {
            ResolvedNetworkPolicy::ProxyFiltered { domains } => domains.clone(),
            _ => vec![],
        };
        let proxy_config = ProxyConfig::with_allowed_domains(domains);
        match ProxyManager::new(proxy_config) {
            Ok(mut manager) => {
                if let Err(e) = manager.start() {
                    eprintln!("[WARN] Failed to start network proxy: {}", e);
                    None
                } else {
                    crate::info_log!("[INFO] Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
                             manager.http_port(), manager.socks5_port());
                    Some(manager)
                }
            }
            Err(e) => {
                eprintln!("[WARN] Failed to create network proxy: {}", e);
                None
            }
        }
    } else if security_policy::is_allow_all_network(&network_policy) {
        crate::info_log!("[INFO] Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    };

    // Generate sandbox profile with proxy ports if available
    let profile_path = work_dir.join("sandbox.sb");
    let profile_content = generate_sandbox_profile_with_proxy(
        skill_dir,
        &runtime.env_dir,
        config,
        work_dir,
        proxy_manager.as_ref().and_then(|m| m.http_port()),
        proxy_manager.as_ref().and_then(|m| m.socks5_port()),
        &network_policy,
    )?;
    fs::write(&profile_path, &profile_content)?;

    // Prepare command based on language
    let (executable, mut args) = match language.as_str() {
        "python" => {
            (runtime.python.clone(), vec![entry_point.to_string()])
        }
        "node" => {
            (runtime.node.clone(), vec![entry_point.to_string()])
        }
        _ => {
            anyhow::bail!("Unsupported language: {}", language);
        }
    };

    // Add script arguments from SKILLBOX_SCRIPT_ARGS environment variable
    // This allows passing CLI arguments to scripts that use argparse
    if let Ok(script_args) = std::env::var("SKILLBOX_SCRIPT_ARGS") {
        if !script_args.is_empty() {
            // Split by whitespace - arguments should not contain spaces
            for arg in script_args.split_whitespace() {
                args.push(arg.to_string());
            }
        }
    }

    // Build sandbox-exec command - directly execute interpreter (no script injection)
    let mut cmd = Command::new("sandbox-exec");
    cmd.args(["-f", profile_path.to_str().expect("profile path must be valid UTF-8")]);
    cmd.arg(&executable);
    cmd.args(&args);

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set environment variables
    cmd.env("SKILLBOX_SANDBOX", "1");
    cmd.env("TMPDIR", work_dir);
    // Expose output dir for skills that save files (e.g. csdn-article, xiaohongshu-writer)
    if let Some(ref output_dir) = crate::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }
    // Do not override HOME - some libs (fonts, cache) need it for path lookup

    // Set NODE_PATH for Node.js
    if language == "node" {
        if let Some(ref node_modules) = runtime.node_modules {
            cmd.env("NODE_PATH", node_modules);
        }
    }

    // Set proxy environment variables if proxy is running
    if let Some(ref manager) = proxy_manager {
        for (key, value) in manager.get_proxy_env_vars() {
            cmd.env(&key, &value);
        }
    }

    // Apply resource limits using pre_exec (pure Rust, no script injection)
    // Use limits from env (SKILLBOX_TIMEOUT_SECS, SKILLBOX_MAX_MEMORY_MB) so RLIMIT_CPU
    // matches user's EXECUTION_TIMEOUT (e.g. 120s). Previously hardcoded 30s caused early kill.
    let memory_limit_mb = limits.max_memory_mb;
    let cpu_limit_secs = limits.timeout_secs;
    let file_size_limit_mb = DEFAULT_FILE_SIZE_LIMIT_MB;
    let max_processes = DEFAULT_MAX_PROCESSES;
    
    unsafe {
        cmd.pre_exec(move || {
            use nix::libc::{rlimit, setrlimit, RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NPROC};
            
            // Memory limit - from SKILLBOX_MAX_MEMORY_MB
            let memory_limit_bytes = memory_limit_mb * 1024 * 1024;
            let mem_limit = rlimit {
                rlim_cur: memory_limit_bytes,
                rlim_max: memory_limit_bytes,
            };
            setrlimit(RLIMIT_AS, &mem_limit);
            
            // CPU time limit - from SKILLBOX_TIMEOUT_SECS (matches EXECUTION_TIMEOUT)
            let cpu_limit = rlimit {
                rlim_cur: cpu_limit_secs,
                rlim_max: cpu_limit_secs,
            };
            setrlimit(RLIMIT_CPU, &cpu_limit);
            
            // File size limit - 10 MB (sufficient for skill outputs; increase if needed for large images)
            let file_limit_bytes = file_size_limit_mb * 1024 * 1024;
            let file_limit = rlimit {
                rlim_cur: file_limit_bytes,
                rlim_max: file_limit_bytes,
            };
            setrlimit(RLIMIT_FSIZE, &file_limit);
            
            // Max processes (fork bomb protection) - 50 processes
            let nproc_limit = rlimit {
                rlim_cur: max_processes,
                rlim_max: max_processes,
            };
            setrlimit(RLIMIT_NPROC, &nproc_limit);
            
            Ok(())
        });
    }

    // Spawn the process
    crate::info_log!("[INFO] sandbox-exec: spawning...");
    let mut child = cmd.spawn().with_context(|| "Failed to spawn sandbox-exec")?;
    crate::info_log!("[INFO] sandbox-exec: writing stdin...");

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }
    crate::info_log!("[INFO] sandbox-exec: waiting for child (timeout {}s)...", limits.timeout_secs);

    // Wait with timeout and memory monitoring
    let memory_limit_bytes = limits.max_memory_bytes();
    let (stdout, stderr, exit_code, was_killed, kill_reason) = 
        wait_with_timeout(&mut child, limits.timeout_secs, memory_limit_bytes, true)?;

    // Check if sandbox-exec itself failed
    if exit_code == 1 && stderr.is_empty() && stdout.is_empty() && !was_killed {
        anyhow::bail!("sandbox-exec failed to execute");
    }

    // Log if process was killed
    if was_killed {
        if let Some(reason) = &kill_reason {
            eprintln!("[SECURITY] Process terminated due to: {}", reason);
        }
    }

    // Proxy manager will be dropped here, stopping the proxy servers
    drop(proxy_manager);

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Generate a Seatbelt sandbox profile for macOS with network proxy support
///
/// Security controls from canonical security_policy (allow-default with explicit deny):
/// 1. MANDATORY DENY: Always block writes to shell configs, git hooks, IDE configs, etc.
/// 2. MOVE PROTECTION: Block file movement to prevent bypass via mv/rename (P0)
/// 3. NETWORK: Route through proxy when enabled, block all when disabled
/// 4. FILE READ: Block sensitive files (/etc, ~/.ssh, etc.)
/// 5. FILE WRITE: Block writes outside work directory
/// 6. PROCESS: Block execution of dangerous commands
/// 7. LOGTAG: Embed unique tag in deny rules for precise violation tracking (P1)
fn generate_sandbox_profile_with_proxy(
    skill_dir: &Path,
    env_path: &Path,
    config: &SandboxConfig,
    work_dir: &Path,
    http_proxy_port: Option<u16>,
    socks5_proxy_port: Option<u16>,
    network_policy: &ResolvedNetworkPolicy,
) -> Result<String> {
    let skill_dir_str = skill_dir.to_string_lossy();
    let work_dir_str = work_dir.to_string_lossy();

    let relaxed = security_policy::is_relaxed_mode();
    let allow_playwright = security_policy::should_allow_playwright();
    
    // Generate unique log tag for this execution (P1: precise violation tracking)
    let command_desc = format!("skill:{}", config.name);
    let log_tag = generate_log_tag(&command_desc);

    let mut profile = String::new();

    // Version declaration with log tag for violation tracking
    profile.push_str("(version 1)\n\n");
    profile.push_str(&format!("; LogTag: {}\n", log_tag));
    profile.push_str(&format!("; SessionSuffix: {}\n\n", get_session_suffix()));

    // ============================================================
    // SECURITY: Mandatory deny paths - ALWAYS blocked, even in allowed dirs
    // These protect against sandbox escapes and configuration tampering
    // ============================================================
    profile.push_str("; SECURITY: Mandatory deny paths (auto-protected files)\n");
    profile.push_str("; These are ALWAYS blocked from writes, even within allowed paths\n");
    profile.push_str("; Includes: shell configs, git hooks, IDE settings, package manager configs,\n");
    profile.push_str(";           security files (.ssh, .aws, etc.), and AI agent configs\n");
    for pattern in generate_seatbelt_mandatory_deny_patterns() {
        // Add log tag to each deny pattern for tracking
        let pattern_with_tag = if pattern.ends_with(')') {
            // Insert (with message "log_tag") before the closing paren
            let without_close = &pattern[..pattern.len() - 1];
            format!("{}\n  (with message \"{}\"))", without_close, log_tag)
        } else {
            pattern
        };
        profile.push_str(&pattern_with_tag);
        profile.push('\n');
    }
    profile.push('\n');
    
    // ============================================================
    // SECURITY: Move blocking rules - Prevent bypass via mv/rename (P0)
    // Paths from canonical security_policy::get_move_protection_paths()
    // ============================================================
    profile.push_str("; SECURITY: Move blocking rules (prevents bypass via mv/rename)\n");
    profile.push_str("; Blocks moving/renaming protected paths and their ancestor directories\n");
    for rule in generate_move_blocking_rules(&security_policy::get_move_protection_paths(), &log_tag) {
        profile.push_str(&rule);
        profile.push('\n');
    }
    profile.push('\n');

    // ============================================================
    // SECURITY: Block sensitive file reads (from security_policy)
    // ============================================================
    profile.push_str("; SECURITY: Block reading sensitive files\n");
    for rule in crate::sandbox::seatbelt::generate_seatbelt_sensitive_read_deny_rules(relaxed) {
        profile.push_str(&rule);
        profile.push('\n');
    }
    profile.push_str("\n");
    // ============================================================
    // SECURITY: Network isolation with proxy support (from security_policy)
    // ============================================================
    if security_policy::is_network_blocked(network_policy) {
        profile.push_str("; SECURITY: Network access DISABLED\n");
        profile.push_str("(deny network*)\n\n");
    } else if security_policy::is_allow_all_network(network_policy) {
        // Network enabled with wildcard "*" - allow all network access without proxy
        profile.push_str("; SECURITY: Network access ALLOWED (wildcard '*' configured)\n");
        profile.push_str("; All outbound network traffic is permitted\n");
        profile.push_str("(allow network*)\n\n");
    } else if http_proxy_port.is_some() || socks5_proxy_port.is_some() {
        // Network enabled with proxy - allow connections to localhost for proxy
        // macOS Seatbelt requires: (remote tcp "localhost:PORT") format
        profile.push_str("; SECURITY: Network access via PROXY\n");
        profile.push_str("; All outbound traffic should go through the filtering proxy\n");
        profile.push_str(&format!("; HTTP proxy port: {:?}, SOCKS5 proxy port: {:?}\n", 
                                  http_proxy_port, socks5_proxy_port));
        
        // Allow connections to specific proxy ports on localhost
        if let Some(http_port) = http_proxy_port {
            profile.push_str(&format!(
                "(allow network-outbound (remote tcp \"localhost:{}\"))\n",
                http_port
            ));
        }
        if let Some(socks_port) = socks5_proxy_port {
            profile.push_str(&format!(
                "(allow network-outbound (remote tcp \"localhost:{}\"))\n",
                socks_port
            ));
        }
        profile.push_str("\n");
    } else {
        // Network enabled but no proxy configured
        // Block all network access by default for security
        profile.push_str("; SECURITY: Network access BLOCKED (deny-default mode)\n");
        profile.push_str("; Note: All network operations are blocked for security\n");
        profile.push_str("(deny network*)\n\n");
    }

    // ============================================================
    // SECURITY: Block dangerous process execution
    // Relaxed: allow git for version control operations
    // Relaxed: allow Playwright Chromium for browser-based skills (e.g. xiaohongshu-writer HTML screenshot)
    // ============================================================
    if allow_playwright {
        profile.push_str("; Allow Playwright for browser-based skills (L2 or SKILLBOX_ALLOW_PLAYWRIGHT=1)\n");
        profile.push_str("; process-fork: required for subprocess.Popen\n");
        profile.push_str("; process-exec: 1) Playwright driver (node) in venv; 2) Chromium in ms-playwright\n");
        profile.push_str("(allow process-fork)\n");
        // Playwright first spawns driver/node (Node.js) from venv's site-packages/playwright/driver/
        if !env_path.as_os_str().is_empty() && env_path.exists() {
            let env_path_str = env_path.to_string_lossy();
            profile.push_str(&format!(
                "(allow process-exec (subpath \"{}\"))\n",
                env_path_str
            ));
        }
        // Then driver spawns Chromium from ~/Library/Caches/ms-playwright/
        if let Ok(home) = std::env::var("HOME") {
            let playwright_cache = format!("{}/Library/Caches/ms-playwright", home);
            if Path::new(&playwright_cache).exists() {
                profile.push_str(&format!(
                    "(allow process-exec (subpath \"{}\"))\n",
                    playwright_cache
                ));
            }
        }
    }
    profile.push_str("; SECURITY: Block dangerous commands (from security_policy)\n");
    for cmd in security_policy::get_process_exec_denylist(relaxed, HomePathStyle::MacOS) {
        profile.push_str(&format!("(deny process-exec (literal \"{}\"))\n", cmd));
    }
    profile.push_str("\n");

    // ============================================================
    // SECURITY: File write restrictions (deny-default mode)
    // Block ALL file writes by default, then allow specific paths only
    // ============================================================
    profile.push_str("; SECURITY: File write restrictions (deny-default mode)\n");
    profile.push_str("; Block ALL file writes by default\n");
    profile.push_str("(deny file-write*)\n");
    profile.push_str("\n");
    
    // Allow writing to isolated work directory (TMPDIR points here)
    profile.push_str("; Allow writing to isolated work directory\n");
    profile.push_str(&format!(
        "(allow file-write* (subpath \"{}\"))\n",
        work_dir_str
    ));

    // Allow writing to project root (parent of .skills) for skill outputs (e.g. xiaohongshu_thumbnail.png)
    if let Some(project_root) = skill_dir.parent().and_then(|p| p.parent()) {
        let project_root_str = project_root.to_string_lossy();
        if !project_root_str.is_empty() && project_root != skill_dir {
            profile.push_str("; Allow writing to project root for skill outputs\n");
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                project_root_str
            ));
        }
    }
    
    // Allow writing to /var/folders for system temp files (Python, Node.js cache)
    profile.push_str("; Allow writing to /var/folders for system temp files\n");
    profile.push_str("(allow file-write* (subpath \"/var/folders\"))\n");
    profile.push_str("(allow file-write* (subpath \"/private/var/folders\"))\n");
    // L2 relaxed: allow ~/Library/Caches for Playwright, pip cache
    if relaxed {
        profile.push_str("(allow file-write* (regex #\"^/Users/[^/]+/Library/Caches\"))\n");
    }
    profile.push_str("\n");

    // ============================================================
    // ALLOW DEFAULT - For non-file-write operations (process, mach, etc.)
    // Note: file-write* is already denied above, this allows other operations
    // ============================================================
    profile.push_str("; Allow default for runtime compatibility (non-file-write operations)\n");
    profile.push_str("(allow default)\n\n");

    // ============================================================
    // ALLOW: Skill and runtime environment directories
    // skill_dir: skill scripts
    // env_path: skilllite isolated env (Python venv / Node node_modules)
    // work_dir: TMPDIR, Python needs read/write for temp files
    // ============================================================
    profile.push_str("; Allow reading skill directory\n");
    profile.push_str(&format!(
        "(allow file-read* (subpath \"{}\"))\n",
        skill_dir_str
    ));
    profile.push_str("; Allow reading TMPDIR (Python temp files)\n");
    profile.push_str(&format!(
        "(allow file-read* (subpath \"{}\"))\n",
        work_dir_str
    ));

    // env_path: Python venv or Node node_modules cache, must be readable
    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let env_path_str = env_path.to_string_lossy();
        profile.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))\n",
            env_path_str
        ));
    }
    // L2 relaxed: runtime env dirs (Python/Node/Playwright etc.)
    // macOS: ~/Library/Caches (skilllite/envs, pip, playwright, npm)
    // Covers env_path parent and sibling runtime caches
    if relaxed {
        profile.push_str("; L2: runtime env dirs (venv, node_modules, pip, playwright, npm)\n");
        profile.push_str("(allow file-read* (regex #\"^/Users/[^/]+/Library/Caches\"))\n");
        // System runtime (when env_path empty, use system python/node)
        profile.push_str("(allow file-read* (subpath \"/usr\"))\n");
        profile.push_str("(allow file-read* (subpath \"/opt/homebrew\"))\n");
        profile.push_str("(allow file-read* (subpath \"/opt/local\"))\n");
    }
    // Python/Pillow runtime paths (xiaohongshu-writer etc. need read in L2)
    // - /System/Library: dyld, Frameworks, Fonts
    profile.push_str("(allow file-read* (subpath \"/System/Library\"))\n");
    // - /Library: Frameworks, Fonts (python.org install etc.)
    profile.push_str("(allow file-read* (subpath \"/Library\"))\n");
    // - /dev: /dev/null, /dev/urandom (Python basic I/O)
    profile.push_str("(allow file-read* (subpath \"/dev\"))\n");
    // - Timezone: override /etc deny, allow localtime only
    profile.push_str("(allow file-read* (literal \"/private/etc/localtime\"))\n");
    profile.push_str("\n");

    Ok(profile)
}

/// Generate a Seatbelt sandbox profile for macOS (legacy, without proxy)
/// 
/// Security controls (using allow-default with explicit deny):
/// 1. MANDATORY DENY: Always block writes to shell configs, git hooks, IDE configs, etc.
/// 2. NETWORK: Block all network access when disabled
/// 3. FILE READ: Block sensitive files (/etc, ~/.ssh, etc.)
/// 4. FILE WRITE: Block writes outside work directory
/// 5. PROCESS: Block execution of dangerous commands
#[allow(dead_code)]
fn generate_sandbox_profile(
    skill_dir: &Path,
    env_path: &Path,
    config: &SandboxConfig,
    work_dir: &Path,
) -> Result<String> {
    let skill_dir_str = skill_dir.to_string_lossy();
    let work_dir_str = work_dir.to_string_lossy();

    let mut profile = String::new();

    // Version declaration
    profile.push_str("(version 1)\n\n");

    // ============================================================
    // SECURITY: Mandatory deny paths - ALWAYS blocked, even in allowed dirs
    // These protect against sandbox escapes and configuration tampering
    // ============================================================
    profile.push_str("; SECURITY: Mandatory deny paths (auto-protected files)\n");
    profile.push_str("; These are ALWAYS blocked from writes, even within allowed paths\n");
    profile.push_str("; Includes: shell configs, git hooks, IDE settings, package manager configs,\n");
    profile.push_str(";           security files (.ssh, .aws, etc.), and AI agent configs\n");
    for pattern in generate_seatbelt_mandatory_deny_patterns() {
        profile.push_str(&pattern);
        profile.push('\n');
    }
    profile.push('\n');

    // ============================================================
    // SECURITY: Block sensitive file reads BEFORE allow default
    // ============================================================
    profile.push_str("; SECURITY: Block reading sensitive files\n");
    profile.push_str("(deny file-read* (subpath \"/etc\"))\n");
    profile.push_str("(deny file-read* (subpath \"/private/etc\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.ssh\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.aws\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.gnupg\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.kube\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.docker\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.config\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.netrc\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.npmrc\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.pypirc\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.bash_history\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/\\.zsh_history\"))\n");
    profile.push_str("(deny file-read* (regex #\"^/Users/[^/]+/Library/Keychains\"))\n");
    let legacy_relaxed = std::env::var("SKILLBOX_SANDBOX_LEVEL").map(|s| s.trim() == "2").unwrap_or(false);
    if !legacy_relaxed {
        profile.push_str("(deny file-read* (regex #\"/\\.git/\"))\n");
        profile.push_str("(deny file-read* (regex #\"/\\.env$\"))\n");
        profile.push_str("(deny file-read* (regex #\"/\\.env\\.[^/]+$\"))\n");
    }
    profile.push_str("\n");

    // ============================================================
    // SECURITY: Network isolation
    // ============================================================
    if !config.network_enabled {
        profile.push_str("; SECURITY: Network access DISABLED\n");
        profile.push_str("(deny network*)\n\n");
    }

    // ============================================================
    // SECURITY: Block dangerous process execution
    // ============================================================
    profile.push_str("; SECURITY: Block dangerous commands\n");
    profile.push_str("(deny process-exec (literal \"/bin/bash\"))\n");
    profile.push_str("(deny process-exec (literal \"/bin/zsh\"))\n");
    profile.push_str("(deny process-exec (literal \"/bin/sh\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/env\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/curl\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/wget\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/ssh\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/scp\"))\n");
    if !legacy_relaxed {
        profile.push_str("(deny process-exec (literal \"/usr/bin/git\"))\n");
    }
    profile.push_str("(deny process-exec (literal \"/bin/rm\"))\n");
    profile.push_str("(deny process-exec (literal \"/bin/chmod\"))\n");
    profile.push_str("(deny process-exec (literal \"/usr/bin/osascript\"))\n");
    profile.push_str("\n");

    // ============================================================
    // SECURITY: File write restrictions (deny-default mode)
    // Block ALL file writes by default, then allow specific paths only
    // ============================================================
    profile.push_str("; SECURITY: File write restrictions (deny-default mode)\n");
    profile.push_str("; Block ALL file writes by default\n");
    profile.push_str("(deny file-write*)\n");
    profile.push_str("\n");
    
    // Allow writing to isolated work directory (TMPDIR points here)
    profile.push_str("; Allow writing to isolated work directory\n");
    profile.push_str(&format!(
        "(allow file-write* (subpath \"{}\"))\n",
        work_dir_str
    ));

    // Allow writing to project root (parent of .skills) for skill outputs (e.g. xiaohongshu_thumbnail.png)
    if let Some(project_root) = skill_dir.parent().and_then(|p| p.parent()) {
        let project_root_str = project_root.to_string_lossy();
        if !project_root_str.is_empty() && project_root != skill_dir {
            profile.push_str("; Allow writing to project root for skill outputs\n");
            profile.push_str(&format!(
                "(allow file-write* (subpath \"{}\"))\n",
                project_root_str
            ));
        }
    }
    
    // Allow writing to /var/folders for system temp files (Python, Node.js cache)
    profile.push_str("; Allow writing to /var/folders for system temp files\n");
    profile.push_str("(allow file-write* (subpath \"/var/folders\"))\n");
    profile.push_str("(allow file-write* (subpath \"/private/var/folders\"))\n");
    // L2 relaxed: allow ~/Library/Caches for Playwright, pip cache
    if legacy_relaxed {
        profile.push_str("(allow file-write* (regex #\"^/Users/[^/]+/Library/Caches\"))\n");
    }
    profile.push_str("\n");

    // ============================================================
    // ALLOW DEFAULT - For non-file-write operations (process, mach, etc.)
    // Note: file-write* is already denied above, this allows other operations
    // ============================================================
    profile.push_str("; Allow default for runtime compatibility (non-file-write operations)\n");
    profile.push_str("(allow default)\n\n");

    // ============================================================
    // ALLOW: Skill and environment directories
    // ============================================================
    profile.push_str("; Allow reading skill directory\n");
    profile.push_str(&format!(
        "(allow file-read* (subpath \"{}\"))\n",
        skill_dir_str
    ));

    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let env_path_str = env_path.to_string_lossy();
        profile.push_str(&format!(
            "(allow file-read* (subpath \"{}\"))\n",
            env_path_str
        ));
    }
    if legacy_relaxed {
        profile.push_str("(allow file-read* (regex #\"^/Users/[^/]+/Library/Caches\"))\n");
        profile.push_str("(allow file-read* (subpath \"/usr\"))\n");
        profile.push_str("(allow file-read* (subpath \"/opt/homebrew\"))\n");
        profile.push_str("(allow file-read* (subpath \"/opt/local\"))\n");
    }
    profile.push_str("\n");

    if config.network_enabled {
        profile.push_str("; Network access enabled\n");
        if config.network_outbound.is_empty() {
            profile.push_str("(allow network-outbound)\n");
        } else {
            for host in &config.network_outbound {
                if let Some(ips) = resolve_host_to_ips(host) {
                    for ip in ips {
                        profile.push_str(&format!(
                            "(allow network-outbound (remote ip \"{}\"))\n",
                            ip
                        ));
                    }
                }
            }
        }
    }

    Ok(profile)
}

/// Resolve a hostname to IP addresses
fn resolve_host_to_ips(host: &str) -> Option<Vec<String>> {
    // Parse host:port format
    let (hostname, port) = if let Some(idx) = host.rfind(':') {
        let (h, p) = host.split_at(idx);
        (h, p.trim_start_matches(':'))
    } else {
        (host, "443")
    };

    // Handle wildcard domains
    if hostname.starts_with("*.") {
        // For wildcard domains, we can't resolve them directly
        // Return None and the caller should handle this case
        return None;
    }

    // Try to resolve
    let addr = format!("{}:{}", hostname, port);
    match addr.as_str().to_socket_addrs() {
        Ok(addrs) => {
            let ips: Vec<String> = addrs
                .map(|a: std::net::SocketAddr| a.ip().to_string())
                .collect();
            if ips.is_empty() {
                None
            } else {
                Some(ips)
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sandbox_profile() {
        let skill_dir = Path::new("/tmp/test_skill");
        let env_path = Path::new("");
        let work_dir = Path::new("/tmp/work");

        let config = SandboxConfig {
            name: "test".to_string(),
            entry_point: "main.py".to_string(),
            language: "python".to_string(),
            network_enabled: false,
            network_outbound: Vec::new(),
            uses_playwright: false,
        };

        let profile = generate_sandbox_profile(skill_dir, env_path, &config, work_dir)
            .expect("test sandbox profile generation should succeed");

        assert!(profile.contains("(version 1)"));
        // The sandbox profile uses allow-default mode with explicit deny rules
        assert!(profile.contains("/tmp/test_skill"));
        assert!(profile.contains("(deny network*)"));
    }
}
