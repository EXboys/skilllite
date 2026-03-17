#![cfg(target_os = "linux")]

use crate::common::wait_with_timeout;
use crate::network_proxy::{ProxyConfig, ProxyManager};
use crate::runner::{ExecutionResult, ResourceLimits, RuntimePaths, SandboxConfig};
use crate::runtime_resolver::{ResolvedRuntime, RuntimeResolver};
use crate::seatbelt::{generate_firejail_blacklist_args, MANDATORY_DENY_DIRECTORIES};
use crate::security::policy::{self as security_policy, ResolvedNetworkPolicy};
use anyhow::{Context, Result};
use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Execute a skill in a Linux sandbox
/// Sandbox is enabled by default. Set SKILLLITE_NO_SANDBOX=1 to disable.
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
    if skilllite_core::config::SandboxEnvConfig::from_env().no_sandbox {
        tracing::warn!("Sandbox disabled via SKILLLITE_NO_SANDBOX - running without protection");
        return execute_simple_with_limits(skill_dir, runtime, config, input_json, limits);
    }

    match execute_with_seccomp(skill_dir, runtime, config, input_json, limits) {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::info!(
                "Seccomp sandbox failed ({}), trying namespace isolation...",
                e
            );
            match execute_with_namespaces(skill_dir, runtime, config, input_json, limits) {
                Ok(result) => Ok(result),
                Err(e2) => {
                    Err(anyhow::anyhow!(
                        "All sandbox methods failed. Seccomp: {}. Namespace: {}. Set SKILLLITE_NO_SANDBOX=1 to run without sandbox (not recommended).",
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

    let resolved = runtime
        .resolve(language)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    let mut cmd = Command::new(&resolved.interpreter);
    cmd.arg(&entry_point);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set environment variables
    cmd.env("SKILLLITE_SANDBOX", "0")
        .env("SKILLBOX_SANDBOX", "0"); // legacy compat
    cmd.env("TMPDIR", work_dir);

    if !config.network_enabled {
        cmd.env("SKILLLITE_NETWORK_DISABLED", "1")
            .env("SKILLBOX_NETWORK_DISABLED", "1"); // legacy compat
    }

    // Apply resource limits via pre_exec (kernel-enforced, not polling-only)
    let memory_limit_mb = limits.max_memory_mb;
    let cpu_limit_secs = limits.timeout_secs;
    let file_size_limit_mb = crate::common::DEFAULT_FILE_SIZE_LIMIT_MB;
    let max_processes = crate::common::DEFAULT_MAX_PROCESSES;

    unsafe {
        cmd.pre_exec(move || {
            use nix::libc::{rlimit, setrlimit, RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NPROC};

            let memory_limit_bytes = memory_limit_mb * 1024 * 1024;
            let mem_limit = rlimit {
                rlim_cur: memory_limit_bytes,
                rlim_max: memory_limit_bytes,
            };
            setrlimit(RLIMIT_AS, &mem_limit);

            let cpu_limit = rlimit {
                rlim_cur: cpu_limit_secs,
                rlim_max: cpu_limit_secs,
            };
            setrlimit(RLIMIT_CPU, &cpu_limit);

            let file_limit_bytes = file_size_limit_mb * 1024 * 1024;
            let file_limit = rlimit {
                rlim_cur: file_limit_bytes,
                rlim_max: file_limit_bytes,
            };
            setrlimit(RLIMIT_FSIZE, &file_limit);

            let nproc_limit = rlimit {
                rlim_cur: max_processes,
                rlim_max: max_processes,
            };
            setrlimit(RLIMIT_NPROC, &nproc_limit);

            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .with_context(|| "Failed to spawn skill process")?;

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
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    let resolved = runtime
        .resolve(language)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;
    let _program = &resolved.interpreter;
    let _args = vec![entry_point.to_string_lossy().to_string()];

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

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
            &resolved,
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
            &resolved,
            &entry_point,
            work_dir,
            limits,
        );
    }

    // No sandbox tool available
    anyhow::bail!(
        "No sandbox tool available (bwrap or firejail). Install bubblewrap: apt install bubblewrap"
    )
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
    resolved: &ResolvedRuntime,
    entry_point: &Path,
    work_dir: &Path,
    limits: crate::runner::ResourceLimits,
) -> Result<ExecutionResult> {
    let env_path = &runtime.env_dir;
    let interpreter_path = resolve_command_path(&resolved.interpreter);
    let network_policy =
        security_policy::resolve_network_policy(config.network_enabled, &config.network_outbound);

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
                    tracing::warn!("Failed to start network proxy: {}", e);
                    None
                } else {
                    tracing::info!(
                        "Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
                        manager.http_port(),
                        manager.socks5_port()
                    );
                    Some(manager)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create network proxy: {}", e);
                None
            }
        }
    } else if security_policy::is_allow_all_network(&network_policy) {
        tracing::info!("Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    };

    let mut cmd = Command::new(bwrap);

    // Basic isolation
    cmd.args(["--unshare-all"]); // Unshare all namespaces
    cmd.args(["--die-with-parent"]); // Kill sandbox if parent dies

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
    for runtime_root in collect_additional_runtime_roots(&interpreter_path, env_path) {
        let runtime_root_str = runtime_root.to_string_lossy();
        cmd.args(["--ro-bind", &runtime_root_str, &runtime_root_str]);
    }

    // Mount the minimum /etc files required for a working runtime inside the sandbox.
    //
    // • ld.so.cache / ld.so.conf[.d] — dynamic linker; lets the loader find
    //   libraries that live in non-standard paths (e.g. /usr/local/lib on
    //   Debian-based Docker images, where Python's libpython3.x resides).
    //
    // • resolv.conf / nsswitch.conf / hosts — DNS/name resolution; required
    //   when the skill makes network calls (e.g. http-request).  Without
    //   these files the resolver returns SERVFAIL / -3 (name resolution failure)
    //   even though the network namespace is shared.
    for etc_file in &[
        "/etc/ld.so.cache",
        "/etc/ld.so.conf",
        "/etc/ld.so.conf.d",
        "/etc/resolv.conf",
        "/etc/nsswitch.conf",
        "/etc/hosts",
        // CA certificate bundle — required for TLS/HTTPS verification
        "/etc/ssl/certs",
        "/etc/ca-certificates.conf",
    ] {
        if Path::new(etc_file).exists() {
            cmd.args(["--ro-bind", etc_file, etc_file]);
        }
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

    // Create /proc.
    // On bare metal / macOS we can mount a real procfs via --proc.
    // Inside Docker, even with seccomp:unconfined, mounting procfs from a user
    // namespace is blocked by the container runtime (AppArmor / capability
    // restrictions).  Detect the container environment and fall back to an
    // empty directory so Python can still start (it does not require a real
    // /proc for HTTP requests or most common skill workloads).
    let in_container = Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker") || s.contains("containerd") || s.contains("kubepods"))
            .unwrap_or(false);
    if in_container {
        // Empty directory – avoids "Can't mount proc: Operation not permitted"
        cmd.args(["--dir", "/proc"]);
    } else {
        // Real procfs – fully isolated process view on bare-metal / macOS
        cmd.args(["--proc", "/proc"]);
    }

    // Network isolation (from security_policy - aligns with macOS)
    if security_policy::is_network_blocked(&network_policy) {
        cmd.args(["--unshare-net"]);
    } else {
        cmd.args(["--share-net"]);
    }

    // Set environment
    cmd.args(["--setenv", "SKILLLITE_SANDBOX", "1"])
        .args(["--setenv", "SKILLBOX_SANDBOX", "1"]); // legacy compat
    cmd.args(["--setenv", "TMPDIR", "/tmp"]);
    cmd.args(["--setenv", "HOME", "/tmp"]);

    if let Some(ref manager) = proxy_manager {
        for (key, value) in manager.get_proxy_env_vars() {
            cmd.args(["--setenv", &key, &value]);
        }
    }
    for (k, v) in &resolved.extra_env {
        cmd.args(["--setenv", k, v]);
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

    // Generate seccomp BPF filter file for Unix socket blocking.
    //
    // bwrap --seccomp FD expects an *open file descriptor* (integer), not a
    // path.  We open the BPF file here, keep the raw fd alive across the
    // fork (into_raw_fd intentionally leaks ownership), and in pre_exec we
    // dup2 it to the well-known slot 3 so that bwrap can read it.
    let seccomp_filter_path = work_dir.join("seccomp.bpf");
    let seccomp_raw_fd: Option<i32> = match generate_seccomp_bpf_file(&seccomp_filter_path) {
        Err(e) => {
            tracing::warn!("Failed to generate seccomp filter: {}", e);
            None
        }
        Ok(()) => {
            use std::os::unix::io::IntoRawFd;
            match fs::File::open(&seccomp_filter_path) {
                Ok(f) => Some(f.into_raw_fd()),
                Err(e) => {
                    tracing::warn!("Failed to open seccomp BPF file: {}", e);
                    None
                }
            }
        }
    };

    // Only add --seccomp when we successfully opened the BPF file.
    if seccomp_raw_fd.is_some() {
        cmd.args(["--seccomp", "3"]);
    }

    // Add the program and arguments
    cmd.arg("--");
    cmd.arg(&interpreter_path);
    cmd.arg(entry_point);

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Apply resource limits via pre_exec — inherited by bwrap's child process through fork+exec
    let memory_limit_mb = limits.max_memory_mb;
    let cpu_limit_secs = limits.timeout_secs;
    let file_size_limit_mb = crate::common::DEFAULT_FILE_SIZE_LIMIT_MB;
    let max_processes = crate::common::DEFAULT_MAX_PROCESSES;

    unsafe {
        cmd.pre_exec(move || {
            use nix::libc::{
                close, dup2, fcntl, rlimit, setrlimit, FD_CLOEXEC, F_GETFD, F_SETFD, RLIMIT_AS,
                RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NPROC,
            };

            let memory_limit_bytes = memory_limit_mb * 1024 * 1024;
            let mem_limit = rlimit {
                rlim_cur: memory_limit_bytes,
                rlim_max: memory_limit_bytes,
            };
            setrlimit(RLIMIT_AS, &mem_limit);

            let cpu_limit = rlimit {
                rlim_cur: cpu_limit_secs,
                rlim_max: cpu_limit_secs,
            };
            setrlimit(RLIMIT_CPU, &cpu_limit);

            let file_limit_bytes = file_size_limit_mb * 1024 * 1024;
            let file_limit = rlimit {
                rlim_cur: file_limit_bytes,
                rlim_max: file_limit_bytes,
            };
            setrlimit(RLIMIT_FSIZE, &file_limit);

            let nproc_limit = rlimit {
                rlim_cur: max_processes,
                rlim_max: max_processes,
            };
            setrlimit(RLIMIT_NPROC, &nproc_limit);

            // If we have an open seccomp BPF fd, dup2 it to FD 3 and clear
            // CLOEXEC so bwrap can read it after execve.
            if let Some(src_fd) = seccomp_raw_fd {
                const SECCOMP_BWRAP_FD: i32 = 3;
                if dup2(src_fd, SECCOMP_BWRAP_FD) >= 0 {
                    let flags = fcntl(SECCOMP_BWRAP_FD, F_GETFD, 0);
                    fcntl(SECCOMP_BWRAP_FD, F_SETFD, flags & !FD_CLOEXEC);
                    close(src_fd);
                }
            }

            Ok(())
        });
    }

    // Spawn the process
    let mut child = cmd
        .spawn()
        .with_context(|| "Failed to spawn bwrap sandbox")?;

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

    // Proxy manager will be dropped here, stopping the proxy servers
    drop(proxy_manager);

    Ok(ExecutionResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Generate a seccomp BPF filter file for bwrap's `--seccomp` option.
///
/// Blocks the same set of dangerous syscalls as `seccomp.rs::build_sandbox_filter`:
/// ptrace, mount, umount2, keyctl, kexec_load, kexec_file_load, pivot_root, chroot,
/// socket(AF_UNIX), clone(CLONE_NEWUSER), unshare(CLONE_NEWUSER).
fn generate_seccomp_bpf_file(path: &Path) -> Result<()> {
    use std::io::Write;

    #[cfg(target_arch = "x86_64")]
    mod nr {
        pub const SOCKET: u32 = 41;
        pub const PTRACE: u32 = 101;
        pub const MOUNT: u32 = 165;
        pub const UMOUNT2: u32 = 166;
        pub const CLONE: u32 = 56;
        pub const KEYCTL: u32 = 250;
        pub const KEXEC_LOAD: u32 = 246;
        pub const KEXEC_FILE_LOAD: u32 = 320;
        pub const PIVOT_ROOT: u32 = 155;
        pub const CHROOT: u32 = 161;
        pub const UNSHARE: u32 = 272;
    }

    #[cfg(target_arch = "aarch64")]
    mod nr {
        pub const SOCKET: u32 = 198;
        pub const PTRACE: u32 = 117;
        pub const MOUNT: u32 = 40;
        pub const UMOUNT2: u32 = 39;
        pub const CLONE: u32 = 220;
        pub const KEYCTL: u32 = 219;
        pub const KEXEC_LOAD: u32 = 104;
        pub const KEXEC_FILE_LOAD: u32 = 294;
        pub const PIVOT_ROOT: u32 = 41;
        pub const CHROOT: u32 = 51;
        pub const UNSHARE: u32 = 97;
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    mod nr {
        pub const SOCKET: u32 = 0;
        pub const PTRACE: u32 = 0;
        pub const MOUNT: u32 = 0;
        pub const UMOUNT2: u32 = 0;
        pub const CLONE: u32 = 0;
        pub const KEYCTL: u32 = 0;
        pub const KEXEC_LOAD: u32 = 0;
        pub const KEXEC_FILE_LOAD: u32 = 0;
        pub const PIVOT_ROOT: u32 = 0;
        pub const CHROOT: u32 = 0;
        pub const UNSHARE: u32 = 0;
    }

    const AF_UNIX: u32 = 1;
    const CLONE_NEWUSER: u32 = 0x10000000;

    const SECCOMP_RET_ALLOW: u32 = 0x7fff0000;
    const SECCOMP_RET_ERRNO: u32 = 0x00050000;
    const EPERM: u32 = 1;

    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const BPF_ALU: u16 = 0x04;
    const BPF_AND: u16 = 0x50;

    const SECCOMP_DATA_NR: u32 = 0;
    const SECCOMP_DATA_ARGS: u32 = 16;

    type Inst = (u16, u8, u8, u32);
    let deny: Inst = (BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ERRNO | EPERM);
    let allow: Inst = (BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ALLOW);

    let mut filter: Vec<Inst> = Vec::with_capacity(36);

    // Load syscall number
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR));

    // Unconditional blocks
    for syscall in [
        nr::PTRACE,
        nr::MOUNT,
        nr::UMOUNT2,
        nr::KEYCTL,
        nr::KEXEC_LOAD,
        nr::KEXEC_FILE_LOAD,
        nr::PIVOT_ROOT,
        nr::CHROOT,
    ] {
        filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 1, syscall));
        filter.push(deny);
    }

    // socket(AF_UNIX) block
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 3, nr::SOCKET));
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS));
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 1, AF_UNIX));
    filter.push(deny);
    // Reload syscall number
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR));

    // clone(CLONE_NEWUSER) block
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 4, nr::CLONE));
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS));
    filter.push((BPF_ALU | BPF_AND | BPF_K, 0, 0, CLONE_NEWUSER));
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 1, CLONE_NEWUSER));
    filter.push(deny);
    // Reload syscall number
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR));

    // unshare(CLONE_NEWUSER) block
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 4, nr::UNSHARE));
    filter.push((BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS));
    filter.push((BPF_ALU | BPF_AND | BPF_K, 0, 0, CLONE_NEWUSER));
    filter.push((BPF_JMP | BPF_JEQ | BPF_K, 0, 1, CLONE_NEWUSER));
    filter.push(deny);

    // Allow everything else
    filter.push(allow);

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
    resolved: &ResolvedRuntime,
    entry_point: &Path,
    work_dir: &Path,
    _limits: ResourceLimits,
) -> Result<ExecutionResult> {
    let env_path = &runtime.env_dir;
    let interpreter_path = resolve_command_path(&resolved.interpreter);
    let network_policy =
        security_policy::resolve_network_policy(config.network_enabled, &config.network_outbound);

    let proxy_manager = if security_policy::should_use_proxy(&network_policy) {
        let domains = match &network_policy {
            ResolvedNetworkPolicy::ProxyFiltered { domains } => domains.clone(),
            _ => vec![],
        };
        let proxy_config = ProxyConfig::with_allowed_domains(domains);
        match ProxyManager::new(proxy_config) {
            Ok(mut manager) => {
                if let Err(e) = manager.start() {
                    tracing::warn!("Failed to start network proxy: {}", e);
                    None
                } else {
                    tracing::info!(
                        "Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
                        manager.http_port(),
                        manager.socks5_port()
                    );
                    Some(manager)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create network proxy: {}", e);
                None
            }
        }
    } else if security_policy::is_allow_all_network(&network_policy) {
        tracing::info!("Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    };

    let mut cmd = Command::new(firejail);

    // Security options
    cmd.args(["--quiet"]);
    cmd.args(["--noprofile"]); // Don't use default profile
    cmd.args(["--private"]); // New /home and /root
    cmd.args(["--private-tmp"]); // New /tmp
    cmd.args(["--private-dev"]); // Minimal /dev
    cmd.args(["--noroot"]); // No root in sandbox
    cmd.args(["--caps.drop=all"]); // Drop all capabilities
    cmd.args(["--seccomp"]); // Enable seccomp (includes Unix socket blocking)

    // File system restrictions
    cmd.args(["--read-only=/usr"]);
    cmd.args(["--read-only=/lib"]);
    if Path::new("/lib64").exists() {
        cmd.args(["--read-only=/lib64"]);
    }
    for runtime_root in collect_additional_runtime_roots(&interpreter_path, env_path) {
        let runtime_root_str = runtime_root.to_string_lossy();
        cmd.args([&format!("--whitelist={}", runtime_root_str)]);
        cmd.args([&format!("--read-only={}", runtime_root_str)]);
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
        tracing::info!("Network enabled with proxy filtering");
    } else {
        tracing::info!("Network enabled (wildcard or direct)");
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
    cmd.arg(&interpreter_path);
    cmd.arg(entry_point);

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set environment
    cmd.env("SKILLLITE_SANDBOX", "1")
        .env("SKILLBOX_SANDBOX", "1"); // legacy compat
    cmd.env("TMPDIR", work_dir);
    if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }

    // Set proxy environment variables if proxy is running
    if let Some(ref manager) = proxy_manager {
        for (key, value) in manager.get_proxy_env_vars() {
            cmd.env(&key, &value);
        }
    }

    // Spawn the process
    let mut child = cmd
        .spawn()
        .with_context(|| "Failed to spawn firejail sandbox")?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }

    // Wait for completion
    let output = child
        .wait_with_output()
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
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    let resolved = runtime
        .resolve(language)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

    // Create temporary directory for execution
    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    let mut cmd = Command::new(&resolved.interpreter);
    cmd.arg(&entry_point);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }

    // Set up stdin/stdout/stderr
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Set working directory
    cmd.current_dir(skill_dir);

    // Set environment variables
    cmd.env("SKILLLITE_SANDBOX", "1")
        .env("SKILLBOX_SANDBOX", "1"); // legacy compat
    cmd.env("TMPDIR", work_dir);
    if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
        cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
    }

    if !config.network_enabled {
        cmd.env("SKILLLITE_NETWORK_DISABLED", "1")
            .env("SKILLBOX_NETWORK_DISABLED", "1"); // legacy compat
    }

    // Create unshared namespaces
    // This requires root privileges
    unsafe {
        cmd.pre_exec(|| {
            unshare(CloneFlags::CLONE_NEWUTS | CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNET)
                .map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("unshare failed: {}", e))
                })?;
            Ok(())
        });
    }

    // Spawn the process
    let mut child = cmd
        .spawn()
        .with_context(|| "Failed to spawn skill process")?;

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
fn setup_mount_namespace(root_path: &Path, skill_dir: &Path, env_dir: &Path) -> Result<()> {
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

fn resolve_command_path(cmd: &Path) -> PathBuf {
    if cmd.is_absolute() {
        cmd.to_path_buf()
    } else {
        resolve_which(cmd).unwrap_or_else(|| cmd.to_path_buf())
    }
}

fn resolve_which(cmd: &Path) -> Option<PathBuf> {
    Command::new("which").arg(cmd).output().ok().and_then(|o| {
        if o.status.success() {
            let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
        None
    })
}

fn collect_additional_runtime_roots(interpreter: &Path, env_path: &Path) -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    let candidates = [
        Some(interpreter.to_path_buf()),
        (!env_path.as_os_str().is_empty()).then(|| env_path.to_path_buf()),
    ];

    for candidate in candidates.into_iter().flatten() {
        collect_runtime_root_from_path(&candidate, &mut roots);
        if let Ok(canonical) = candidate.canonicalize() {
            collect_runtime_root_from_path(&canonical, &mut roots);
        }
    }

    roots.into_iter().collect()
}

fn collect_runtime_root_from_path(path: &Path, roots: &mut BTreeSet<PathBuf>) {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    collect_runtime_root_from_path_with_home(path, roots, home.as_deref());
}

fn collect_runtime_root_from_path_with_home(
    path: &Path,
    roots: &mut BTreeSet<PathBuf>,
    home: Option<&Path>,
) {
    for prefix in known_system_runtime_prefixes() {
        if path.starts_with(prefix) && prefix.exists() {
            roots.insert(prefix.to_path_buf());
        }
    }

    if let Some(home) = home {
        for root in known_user_runtime_roots(home) {
            if path.starts_with(&root) && root.exists() {
                roots.insert(root);
            }
        }
    }
}

fn known_system_runtime_prefixes() -> Vec<&'static Path> {
    ["/usr/local", "/opt", "/nix", "/snap", "/var/lib/snapd"]
        .into_iter()
        .map(Path::new)
        .collect()
}

fn known_user_runtime_roots(home: &Path) -> Vec<PathBuf> {
    [
        ".pyenv",
        ".asdf",
        ".local",
        "miniconda3",
        "anaconda3",
        "micromamba",
    ]
    .into_iter()
    .map(|suffix| home.join(suffix))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_collect_additional_runtime_roots_detects_common_prefixes() {
        let roots = collect_additional_runtime_roots(
            Path::new("/usr/local/bin/python3"),
            Path::new("/opt/conda/envs/demo"),
        );

        assert!(roots.iter().any(|p| p == Path::new("/usr/local")));
        assert!(roots.iter().any(|p| p == Path::new("/opt")));
    }

    #[test]
    fn test_resolve_command_path_preserves_absolute_paths() {
        let path = Path::new("/usr/bin/python3");
        assert_eq!(resolve_command_path(path), path);
    }

    #[test]
    fn test_collect_runtime_root_from_path_detects_pyenv_layout() {
        let temp_dir = TempDir::new().expect("temp dir");
        let home = temp_dir.path().join("home");
        let pyenv_root = home.join(".pyenv");
        let interpreter = pyenv_root.join("versions/3.12.2/bin/python3");
        std::fs::create_dir_all(interpreter.parent().expect("parent")).expect("create pyenv bin");
        std::fs::create_dir_all(&pyenv_root).expect("create pyenv root");

        let mut roots = BTreeSet::new();
        collect_runtime_root_from_path_with_home(&interpreter, &mut roots, Some(&home));

        assert!(roots.contains(&pyenv_root));
    }

    #[test]
    fn test_collect_runtime_root_from_path_detects_conda_layout() {
        let temp_dir = TempDir::new().expect("temp dir");
        let home = temp_dir.path().join("home");
        let conda_root = home.join("miniconda3");
        let interpreter = conda_root.join("envs/demo/bin/python");
        std::fs::create_dir_all(interpreter.parent().expect("parent")).expect("create conda bin");
        std::fs::create_dir_all(&conda_root).expect("create conda root");

        let mut roots = BTreeSet::new();
        collect_runtime_root_from_path_with_home(&interpreter, &mut roots, Some(&home));

        assert!(roots.contains(&conda_root));
    }

    #[test]
    fn test_known_user_runtime_roots_include_pyenv_and_conda() {
        let home = Path::new("/tmp/fake-home");
        let roots = known_user_runtime_roots(home);

        assert!(roots.iter().any(|root| root == &home.join(".pyenv")));
        assert!(roots.iter().any(|root| root == &home.join("miniconda3")));
    }
}
