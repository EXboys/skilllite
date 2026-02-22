#![cfg(target_os = "linux")]

use crate::common::wait_with_timeout;
use crate::runner::{ExecutionResult, ResourceLimits, RuntimePaths, SandboxConfig};
use crate::network_proxy::{ProxyConfig, ProxyManager};
use crate::security::policy::{self as security_policy, ResolvedNetworkPolicy};
use crate::seatbelt::{generate_firejail_blacklist_args, MANDATORY_DENY_DIRECTORIES};
use anyhow::{Context, Result};
use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Execute a skill in a Linux sandbox
/// Sandbox is enabled by default. Set SKILLBOX_NO_SANDBOX=1 to disable.
pub fn execute(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
) -> Result<ExecutionResult> {
    execute_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        crate::runner::ResourceLimits::default(),
    )
}

/// Execute a skill in a Linux sandbox with custom resource limits
pub fn execute_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    if std::env::var("SKILLBOX_NO_SANDBOX").is_ok() {
        eprintln!("[WARN] Sandbox disabled via SKILLBOX_NO_SANDBOX - running without protection");
        return execute_simple_with_limits(skill_dir, runtime, config, input_json, limits);
    }
    
    match execute_with_seccomp(skill_dir, runtime, config, input_json, limits) {
        Ok(result) => Ok(result),
        Err(e) => {
            eprintln!("[INFO] Seccomp sandbox failed ({}), trying namespace isolation...", e);
            match execute_with_namespaces(skill_dir, runtime, config, input_json, limits) {
                Ok(result) => Ok(result),
                Err(e2) => {
                    Err(anyhow::anyhow!(
                        "All sandbox methods failed. Seccomp: {}. Namespace: {}. Set SKILLBOX_NO_SANDBOX=1 to run without sandbox (not recommended).",
                        e, e2
                    ))
                }
            }
        }
    }
}

/// Simple execution without sandbox (fallback when all sandbox methods fail)
fn execute_simple(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
) -> Result<ExecutionResult> {
    execute_simple_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        crate::runner::ResourceLimits::default(),
    )
}

/// Simple execution without sandbox with custom resource limits
fn execute_simple_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    // Prepare command based on language
    let mut cmd = match language.as_str() {
        "python" => {
            let mut c = Command::new(&runtime.python);
            c.arg(&entry_point);
            c
        }
        "node" => {
            let mut c = Command::new(&runtime.node);
            c.arg(&entry_point);

            if let Some(ref node_modules) = runtime.node_modules {
                c.env("NODE_PATH", node_modules);
            }
            c
        }
        _ => {
            anyhow::bail!("Unsupported language: {}", language);
        }
    };

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

    let mut child = cmd.spawn().with_context(|| "Failed to spawn skill process")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }

    let (stdout, stderr, exit_code, _, _) = wait_with_timeout(
        &mut child,
        limits.timeout_secs,
        limits.max_memory_bytes(),
        true,
    )?;

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Execute with seccomp-based sandbox (works without root privileges)
/// Uses landlock on Linux 5.13+ or falls back to seccomp
fn execute_with_seccomp(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    use std::os::unix::process::CommandExt;
    
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    // Prepare command based on language
    let (program, mut args) = match language.as_str() {
        "python" => {
            (runtime.python.clone(), vec![entry_point.to_string_lossy().to_string()])
        }
        "node" => {
            (runtime.node.clone(), vec![entry_point.to_string_lossy().to_string()])
        }
        _ => {
            anyhow::bail!("Unsupported language: {}", language);
        }
    };

    // Use bwrap (bubblewrap) if available for unprivileged sandboxing
    // bwrap is commonly available on Linux and provides namespace isolation without root
    let bwrap_path = which_bwrap();
    
    if let Some(bwrap) = bwrap_path {
        return execute_with_bwrap(
            &bwrap,
            skill_dir,
            runtime,
            config,
            input_json,
            &program,
            &entry_point,
            work_dir,
            limits,
        );
    }

    // Fallback: Use firejail if available
    let firejail_path = which_firejail();
    if let Some(firejail) = firejail_path {
        return execute_with_firejail(
            &firejail,
            skill_dir,
            runtime,
            config,
            input_json,
            &program,
            &entry_point,
            work_dir,
            limits,
        );
    }

    // No sandbox tool available
    anyhow::bail!("No sandbox tool available (bwrap or firejail). Install bubblewrap: apt install bubblewrap")
}

/// Check if bwrap (bubblewrap) is available
fn which_bwrap() -> Option<std::path::PathBuf> {
    std::process::Command::new("which")
        .arg("bwrap")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(std::path::PathBuf::from(path));
                }
            }
            None
        })
}

/// Check if firejail is available
fn which_firejail() -> Option<std::path::PathBuf> {
    std::process::Command::new("which")
        .arg("firejail")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(std::path::PathBuf::from(path));
                }
            }
            None
        })
}

/// Execute with bubblewrap (bwrap) sandbox with network proxy support
fn execute_with_bwrap(
    bwrap: &Path,
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    program: &Path,
    entry_point: &Path,
    work_dir: &Path,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    let env_path = &runtime.env_dir;
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
                    eprintln!("[INFO] Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
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
        eprintln!("[INFO] Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    };

    let mut cmd = Command::new(bwrap);
    
    // Basic isolation
    cmd.args(["--unshare-all"]);  // Unshare all namespaces
    cmd.args(["--die-with-parent"]);  // Kill sandbox if parent dies
    
    // Mount minimal filesystem
    cmd.args(["--ro-bind", "/usr", "/usr"]);
    cmd.args(["--ro-bind", "/lib", "/lib"]);
    if Path::new("/lib64").exists() {
        cmd.args(["--ro-bind", "/lib64", "/lib64"]);
    }
    cmd.args(["--ro-bind", "/bin", "/bin"]);
    if Path::new("/sbin").exists() {
        cmd.args(["--ro-bind", "/sbin", "/sbin"]);
    }
    
    // Mount skill directory as read-only
    let skill_dir_str = skill_dir.to_string_lossy();
    cmd.args(["--ro-bind", &skill_dir_str, &skill_dir_str]);
    
    // Create empty home with --dir /home first, then bind env_path so Python/Node env is readable
    cmd.args(["--dir", "/home"]);
    cmd.args(["--dir", "/root"]);
    
    // Mount environment directory (Python venv / Node node_modules) - must be after --dir or it gets overwritten
    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let env_path_str = env_path.to_string_lossy();
        cmd.args(["--ro-bind", &env_path_str, &env_path_str]);
    }
    
    let relaxed = security_policy::is_relaxed_mode();
    if relaxed {
        if let Ok(home) = std::env::var("HOME") {
            let cache = std::path::Path::new(&home).join(".cache");
            if cache.exists() {
                let cache_str = cache.to_string_lossy();
                cmd.args(["--ro-bind", &cache_str, &cache_str]);
            }
        }
    }
    
    // Mount work directory as read-write
    let work_dir_str = work_dir.to_string_lossy();
    cmd.args(["--bind", &work_dir_str, "/tmp"]);
    
    // Create minimal /dev
    cmd.args(["--dev", "/dev"]);
    
    // Create /proc (needed for Python)
    cmd.args(["--proc", "/proc"]);
    
    // Network isolation (from security_policy - aligns with macOS)
    if security_policy::is_network_blocked(&network_policy) {
        cmd.args(["--unshare-net"]);
    } else {
        cmd.args(["--share-net"]);
    }
    
    // Set environment
    cmd.args(["--setenv", "SKILLBOX_SANDBOX", "1"]);
    cmd.args(["--setenv", "TMPDIR", "/tmp"]);
    cmd.args(["--setenv", "HOME", "/tmp"]);
    
    if let Some(ref manager) = proxy_manager {
        for (key, value) in manager.get_proxy_env_vars() {
            cmd.args(["--setenv", &key, &value]);
        }
    }
    
    // Block mandatory deny directories using tmpfs (makes them empty)
    for dir in MANDATORY_DENY_DIRECTORIES {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let full_path = if dir.starts_with('/') {
            dir.to_string()
        } else {
            format!("{}/{}", home, dir)
        };
        // Use tmpfs to hide sensitive directories
        if Path::new(&full_path).exists() {
            cmd.args(["--tmpfs", &full_path]);
        }
    }
    
    // Generate seccomp BPF filter file for Unix socket blocking
    let seccomp_filter_path = work_dir.join("seccomp.bpf");
    if let Err(e) = generate_seccomp_bpf_file(&seccomp_filter_path) {
        eprintln!("[WARN] Failed to generate seccomp filter: {}", e);
    } else {
        // Apply seccomp filter via bwrap
        let filter_path_str = seccomp_filter_path.to_string_lossy();
        cmd.args(["--seccomp", "3", &filter_path_str]);
    }
    
    // Add the program and arguments
    cmd.arg("--");
    cmd.arg(program);
    cmd.arg(entry_point);
    
    // Set working directory
    cmd.current_dir(skill_dir);
    
    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    // Spawn the process
    let mut child = cmd.spawn().with_context(|| "Failed to spawn bwrap sandbox")?;
    
    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }
    
    // Wait for completion
    let output = child.wait_with_output()
        .with_context(|| "Failed to wait for bwrap sandbox")?;
    
    // Proxy manager will be dropped here, stopping the proxy servers
    drop(proxy_manager);
    
    Ok(ExecutionResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Generate a seccomp BPF filter file that blocks Unix socket creation
fn generate_seccomp_bpf_file(path: &Path) -> Result<()> {
    use std::io::Write;
    
    // BPF filter that blocks socket(AF_UNIX, ...) syscalls
    // This is a binary BPF program format that bwrap can load
    
    // For x86_64: socket syscall is 41, AF_UNIX is 1
    // For aarch64: socket syscall is 198, AF_UNIX is 1
    
    #[cfg(target_arch = "x86_64")]
    const SYS_SOCKET: u32 = 41;
    
    #[cfg(target_arch = "aarch64")]
    const SYS_SOCKET: u32 = 198;
    
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    const SYS_SOCKET: u32 = 0;
    
    const AF_UNIX: u32 = 1;
    
    // Seccomp return values
    const SECCOMP_RET_ALLOW: u32 = 0x7fff0000;
    const SECCOMP_RET_ERRNO: u32 = 0x00050000;
    const EPERM: u32 = 1;
    
    // BPF instruction codes
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    
    // Seccomp data offsets
    const SECCOMP_DATA_NR: u32 = 0;
    const SECCOMP_DATA_ARGS: u32 = 16;
    
    // Build BPF filter instructions
    let filter: Vec<(u16, u8, u8, u32)> = vec![
        // Load syscall number
        (BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR),
        // If not socket(), allow (jump 3 instructions forward)
        (BPF_JMP | BPF_JEQ | BPF_K, 0, 3, SYS_SOCKET),
        // Load first argument (domain/family)
        (BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS),
        // If AF_UNIX, return EPERM (jump 0 forward to next instruction)
        (BPF_JMP | BPF_JEQ | BPF_K, 0, 1, AF_UNIX),
        // Return EPERM for AF_UNIX sockets
        (BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ERRNO | EPERM),
        // Allow everything else
        (BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ALLOW),
    ];
    
    // Write filter to file in binary format
    let mut file = fs::File::create(path)?;
    for (code, jt, jf, k) in filter {
        file.write_all(&code.to_ne_bytes())?;
        file.write_all(&[jt])?;
        file.write_all(&[jf])?;
        file.write_all(&k.to_ne_bytes())?;
    }
    
    Ok(())
}

/// Execute with firejail sandbox with network proxy support
fn execute_with_firejail(
    firejail: &Path,
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    program: &Path,
    entry_point: &Path,
    work_dir: &Path,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    let env_path = &runtime.env_dir;
    let network_policy = security_policy::resolve_network_policy(
        config.network_enabled,
        &config.network_outbound,
    );

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
                    eprintln!("[INFO] Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
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
        eprintln!("[INFO] Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    };

    let mut cmd = Command::new(firejail);
    
    // Security options
    cmd.args(["--quiet"]);
    cmd.args(["--noprofile"]);  // Don't use default profile
    cmd.args(["--private"]);  // New /home and /root
    cmd.args(["--private-tmp"]);  // New /tmp
    cmd.args(["--private-dev"]);  // Minimal /dev
    cmd.args(["--noroot"]);  // No root in sandbox
    cmd.args(["--caps.drop=all"]);  // Drop all capabilities
    cmd.args(["--seccomp"]);  // Enable seccomp (includes Unix socket blocking)
    
    // File system restrictions
    cmd.args(["--read-only=/usr"]);
    cmd.args(["--read-only=/lib"]);
    if Path::new("/lib64").exists() {
        cmd.args(["--read-only=/lib64"]);
    }
    
    // Whitelist skill directory (read-only)
    let skill_dir_str = skill_dir.to_string_lossy();
    cmd.args([&format!("--whitelist={}", skill_dir_str)]);
    cmd.args([&format!("--read-only={}", skill_dir_str)]);
    
    // Whitelist environment directory if exists
    if !env_path.as_os_str().is_empty() && env_path.exists() {
        let env_path_str = env_path.to_string_lossy();
        cmd.args([&format!("--whitelist={}", env_path_str)]);
        cmd.args([&format!("--read-only={}", env_path_str)]);
    }
    
    // Network isolation (from security_policy - aligns with macOS)
    if security_policy::is_network_blocked(&network_policy) {
        cmd.args(["--net=none"]);
    } else if proxy_manager.is_some() {
        eprintln!("[INFO] Network enabled with proxy filtering");
    } else {
        eprintln!("[INFO] Network enabled (wildcard or direct)");
    }
    
    // Block sensitive directories using mandatory deny list from security module
    cmd.args(["--blacklist=/etc/passwd"]);
    cmd.args(["--blacklist=/etc/shadow"]);
    
    // Add all mandatory deny paths from security module
    for arg in generate_firejail_blacklist_args() {
        cmd.arg(&arg);
    }
    
    // Add the program and arguments
    cmd.arg("--");
    cmd.arg(program);
    cmd.arg(entry_point);
    
    // Set working directory
    cmd.current_dir(skill_dir);
    
    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    // Set environment
    cmd.env("SKILLBOX_SANDBOX", "1");
    cmd.env("TMPDIR", work_dir);
    if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }
    
    // Set proxy environment variables if proxy is running
    if let Some(ref manager) = proxy_manager {
        for (key, value) in manager.get_proxy_env_vars() {
            cmd.env(&key, &value);
        }
    }
    
    // Spawn the process
    let mut child = cmd.spawn().with_context(|| "Failed to spawn firejail sandbox")?;
    
    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }
    
    // Wait for completion
    let output = child.wait_with_output()
        .with_context(|| "Failed to wait for firejail sandbox")?;
    
    // Proxy manager will be dropped here, stopping the proxy servers
    drop(proxy_manager);
    
    Ok(ExecutionResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

/// Execute with namespace isolation (requires root)
fn execute_with_namespaces(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    use std::os::unix::process::CommandExt;
    
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    // Prepare command based on language
    let mut cmd = match language.as_str() {
        "python" => {
            let mut c = Command::new(&runtime.python);
            c.arg(&entry_point);
            c
        }
        "node" => {
            let mut c = Command::new(&runtime.node);
            c.arg(&entry_point);
            c
        }
        _ => {
            anyhow::bail!("Unsupported language: {}", language);
        }
    };

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set environment variables
    cmd.env("SKILLBOX_SANDBOX", "1");
    cmd.env("TMPDIR", work_dir);
    if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }

    if !config.network_enabled {
        cmd.env("SKILLBOX_NETWORK_DISABLED", "1");
    }

    // Create unshared namespaces
    // This requires root privileges
    unsafe {
        cmd.pre_exec(|| {
            unshare(CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNET)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("unshare failed: {}", e)))?;
            Ok(())
        });
    }

    // Spawn the process
    let mut child = cmd.spawn().with_context(|| "Failed to spawn skill process")?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }

    // Wait with timeout and memory monitoring
    let (stdout, stderr, exit_code, _, _) = wait_with_timeout(
        &mut child,
        limits.timeout_secs,
        limits.max_memory_bytes(),
        true,
    )?;

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
    })
}
/// Set up mount namespace with read-only binds
#[allow(dead_code)]
fn setup_mount_namespace(
    root_path: &Path,
    skill_dir: &Path,
    env_dir: &Path,
) -> Result<()> {
    // Create necessary directories
    fs::create_dir_all(root_path.join("usr"))?;
    fs::create_dir_all(root_path.join("lib"))?;
    fs::create_dir_all(root_path.join("lib64"))?;
    fs::create_dir_all(root_path.join("tmp"))?;
    fs::create_dir_all(root_path.join("skill"))?;
    fs::create_dir_all(root_path.join("env"))?;

    // Bind mount system directories as read-only
    let readonly_flags = MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_REC;

    mount(
        Some(Path::new("/usr")),
        &root_path.join("usr"),
        None::<&str>,
        readonly_flags,
        None::<&str>,
    )?;

    mount(
        Some(Path::new("/lib")),
        &root_path.join("lib"),
        None::<&str>,
        readonly_flags,
        None::<&str>,
    )?;

    if Path::new("/lib64").exists() {
        mount(
            Some(Path::new("/lib64")),
            &root_path.join("lib64"),
            None::<&str>,
            readonly_flags,
            None::<&str>,
        )?;
    }

    // Bind mount skill directory as read-only
    mount(
        Some(skill_dir),
        &root_path.join("skill"),
        None::<&str>,
        readonly_flags,
        None::<&str>,
    )?;

    // Bind mount environment as read-only
    if !env_dir.as_os_str().is_empty() && env_dir.exists() {
        mount(
            Some(env_dir),
            &root_path.join("env"),
            None::<&str>,
            readonly_flags,
            None::<&str>,
        )?;
    }

    // Mount tmpfs for /tmp
    mount(
        None::<&str>,
        &root_path.join("tmp"),
        Some("tmpfs"),
        MsFlags::empty(),
        Some("size=100M"),
    )?;

    Ok(())
}
