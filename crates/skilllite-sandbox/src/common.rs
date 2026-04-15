//! Common utilities for sandbox implementations
//!
//! This module provides shared functionality used by both macOS and Linux
//! sandbox implementations, including process monitoring and resource limits.

use anyhow::Context;

use crate::Result;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::network_proxy::{ProxyConfig, ProxyManager};
use crate::runner::{ExecutionResult, ResourceLimits};
use crate::security::policy::{self as security_policy, ResolvedNetworkPolicy};

// ============================================================
// Environment Variable Compatibility Layer
// ============================================================

/// Read an environment variable with backward-compatible fallback.
/// Checks `SKILLLITE_*` (new name) first, then `SKILLBOX_*` (legacy name).
pub fn env_compat(new_key: &str, old_key: &str) -> std::result::Result<String, std::env::VarError> {
    std::env::var(new_key).or_else(|_| std::env::var(old_key))
}

/// Check if an environment variable is set (new name or legacy name).
pub fn env_compat_is_set(new_key: &str, old_key: &str) -> bool {
    std::env::var(new_key).is_ok() || std::env::var(old_key).is_ok()
}

// ============================================================
// Resource Limits Constants (Single Source of Truth)
// ============================================================

/// Default maximum memory limit in MB
pub const DEFAULT_MAX_MEMORY_MB: u64 = 256;

/// Default execution timeout in seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default file size limit in MB
pub const DEFAULT_FILE_SIZE_LIMIT_MB: u64 = 10;

/// Maximum number of processes (fork bomb protection).
/// On macOS, RLIMIT_NPROC is per-UID (counts ALL user processes), so the
/// default must be high enough to accommodate existing processes + skill children.
/// Override via SKILLLITE_MAX_PROCESSES env var.
#[cfg(target_os = "macos")]
pub const DEFAULT_MAX_PROCESSES: u64 = 512;

#[cfg(not(target_os = "macos"))]
pub const DEFAULT_MAX_PROCESSES: u64 = 50;

/// Read the effective max-processes limit, honoring env override.
pub fn effective_max_processes() -> u64 {
    std::env::var("SKILLLITE_MAX_PROCESSES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_MAX_PROCESSES)
}

/// Memory check interval in milliseconds
pub const MEMORY_CHECK_INTERVAL_MS: u64 = 100;

/// Grace period in seconds after SIGTERM before SIGKILL (progressive timeout, Unix only)
const TIMEOUT_GRACE_SECS: u64 = 2;

/// Get memory usage of a process in bytes (platform-specific implementation)
/// Returns None if memory information cannot be retrieved
#[cfg(target_os = "macos")]
pub fn get_process_memory(pid: u32) -> Option<u64> {
    use std::process::Command;

    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;

    if output.status.success() {
        let rss_str = String::from_utf8_lossy(&output.stdout);
        // ps returns RSS in KB, convert to bytes
        if let Ok(rss_kb) = rss_str.trim().parse::<u64>() {
            return Some(rss_kb * 1024);
        }
    }

    None
}

/// Get memory usage of a process in bytes (Linux version)
/// Uses /proc/<pid>/status to read VmRSS
#[cfg(target_os = "linux")]
pub fn get_process_memory(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;

    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(rss_kb) = parts[1].parse::<u64>() {
                    return Some(rss_kb * 1024);
                }
            }
            break;
        }
    }

    None
}

/// Get memory usage of a process in bytes (Windows version)
/// Uses tasklist command to get working set size
#[cfg(target_os = "windows")]
pub fn get_process_memory(pid: u32) -> Option<u64> {
    use std::process::Command;

    // Use tasklist to get memory info
    // Format: tasklist /FI "PID eq <pid>" /FO CSV /NH
    let mut cmd = Command::new("tasklist");
    hide_child_console(&mut cmd);
    let output = cmd
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        // CSV format: "Image Name","PID","Session Name","Session#","Mem Usage"
        // Example: "python.exe","1234","Console","1","50,000 K"
        for line in output_str.lines() {
            if line.contains(&pid.to_string()) {
                // Parse the memory field (last column)
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 5 {
                    // Remove quotes and "K" suffix, handle comma in numbers
                    let mem_str = parts[4]
                        .trim()
                        .trim_matches('"')
                        .replace(" K", "")
                        .replace(",", "");
                    if let Ok(mem_kb) = mem_str.parse::<u64>() {
                        return Some(mem_kb * 1024);
                    }
                }
            }
        }
    }

    None
}

/// Wait for child process with timeout and memory monitoring
///
/// This function monitors a child process and enforces resource limits:
/// - Timeout: kills the process if it exceeds the specified duration
/// - Memory limit: kills the process if RSS exceeds the specified bytes
///
/// Stdin: Closes child stdin at start so the process sees EOF and does not block on read.
/// Callers should have already written input; this is a safety measure.
///
/// Timeout: On Unix, uses progressive timeout (SIGTERM first, then SIGKILL after a short grace).
///
/// IMPORTANT: Reads stdout/stderr in background threads while the process runs.
/// Without this, a child writing large output (>64KB pipe buffer) would block
/// on write, and we'd deadlock waiting for the child to exit.
///
/// # Arguments
/// * `child` - The child process to monitor
/// * `timeout_secs` - Maximum execution time in seconds
/// * `memory_limit_bytes` - Maximum memory usage in bytes
/// * `stream_stderr` - If true, forward child stderr to parent stderr in real-time (shows progress)
///
/// # Returns
/// A tuple of (stdout, stderr, exit_code, was_killed, kill_reason)
pub fn wait_with_timeout(
    child: &mut Child,
    timeout_secs: u64,
    memory_limit_bytes: u64,
    stream_stderr: bool,
) -> Result<(String, String, i32, bool, Option<String>)> {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let check_interval = Duration::from_millis(MEMORY_CHECK_INTERVAL_MS);

    // Close stdin so the child sees EOF and does not block waiting for input.
    let _ = child.stdin.take();

    // Spawn threads to read stdout/stderr *while* the process runs.
    // Otherwise large output (>pipe buffer ~64KB) blocks the child and we deadlock.
    let stdout_handle = child.stdout.take().map(|mut out| {
        thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        })
    });
    let stderr_handle = child.stderr.take().map(|mut err| {
        thread::spawn(move || {
            use std::io::Write;
            let mut s = String::new();
            let mut buf = [0u8; 4096];
            loop {
                match err.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]);
                        s.push_str(&chunk);
                        if stream_stderr {
                            let _ = std::io::stderr().write_all(&buf[..n]);
                            let _ = std::io::stderr().flush();
                        }
                    }
                    Err(_) => break,
                }
            }
            s
        })
    });

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_handle
                    .map(|h| h.join().unwrap_or_default())
                    .unwrap_or_default();
                let stderr = stderr_handle
                    .map(|h| h.join().unwrap_or_default())
                    .unwrap_or_default();

                // Post-exit memory check via getrusage(RUSAGE_CHILDREN).
                // On macOS RLIMIT_AS is not enforced by the kernel, so a
                // fast-allocating script can finish before the RSS polling
                // loop catches it. ru_maxrss gives the peak RSS the child
                // ever reached and lets us reject the result retroactively.
                if let Some(peak) = get_children_peak_rss_bytes() {
                    if peak > memory_limit_bytes {
                        let peak_mb = peak / (1024 * 1024);
                        let limit_mb = memory_limit_bytes / (1024 * 1024);
                        return Ok((
                            String::new(),
                            format!(
                                "Process rejected: peak memory ({} MB) exceeded limit ({} MB)",
                                peak_mb, limit_mb
                            ),
                            -1,
                            true,
                            Some("memory_limit".to_string()),
                        ));
                    }
                }

                return Ok((stdout, stderr, status.code().unwrap_or(-1), false, None));
            }
            Ok(None) => {}
            Err(e) => {
                let _ = stdout_handle.map(|h| h.join());
                let _ = stderr_handle.map(|h| h.join());
                return Err(crate::Error::validation(format!(
                    "Failed to wait for process: {}",
                    e
                )));
            }
        }

        if start.elapsed() > timeout {
            kill_with_progressive_timeout(child, stdout_handle, stderr_handle);
            return Ok((
                String::new(),
                format!(
                    "Process killed: exceeded timeout of {} seconds",
                    timeout_secs
                ),
                -1,
                true,
                Some("timeout".to_string()),
            ));
        }

        if let Some(memory) = get_process_memory(child.id()) {
            if memory > memory_limit_bytes {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_handle.map(|h| h.join());
                let _ = stderr_handle.map(|h| h.join());
                let memory_mb = memory / (1024 * 1024);
                let limit_mb = memory_limit_bytes / (1024 * 1024);
                return Ok((
                    String::new(),
                    format!(
                        "Process killed: memory usage ({} MB) exceeded limit ({} MB)",
                        memory_mb, limit_mb
                    ),
                    -1,
                    true,
                    Some("memory_limit".to_string()),
                ));
            }
        }

        thread::sleep(check_interval);
    }
}

/// Kill child and join stdout/stderr reader threads.
/// On Unix: sends SIGTERM first, waits up to TIMEOUT_GRACE_SECS, then SIGKILL (progressive timeout).
#[cfg(unix)]
fn kill_with_progressive_timeout(
    child: &mut Child,
    stdout_handle: Option<thread::JoinHandle<String>>,
    stderr_handle: Option<thread::JoinHandle<String>>,
) {
    let pid = nix::unistd::Pid::from_raw(child.id() as i32);
    let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
    let grace = Duration::from_secs(TIMEOUT_GRACE_SECS);
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if child.try_wait().ok().and_then(|s| s).is_some() {
            let _ = stdout_handle.map(|h| h.join());
            let _ = stderr_handle.map(|h| h.join());
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.map(|h| h.join());
    let _ = stderr_handle.map(|h| h.join());
}

#[cfg(not(unix))]
fn kill_with_progressive_timeout(
    child: &mut Child,
    stdout_handle: Option<thread::JoinHandle<String>>,
    stderr_handle: Option<thread::JoinHandle<String>>,
) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = stdout_handle.map(|h| h.join());
    let _ = stderr_handle.map(|h| h.join());
}

/// Get peak RSS of all waited-for children via getrusage(RUSAGE_CHILDREN).
/// Returns bytes on all platforms (macOS reports bytes, Linux reports KB).
#[cfg(unix)]
fn get_children_peak_rss_bytes() -> Option<u64> {
    use nix::libc::{getrusage, rusage, RUSAGE_CHILDREN};
    let mut usage: rusage = unsafe { std::mem::zeroed() };
    let ret = unsafe { getrusage(RUSAGE_CHILDREN, &mut usage) };
    if ret != 0 {
        return None;
    }
    let maxrss = usage.ru_maxrss;
    if maxrss <= 0 {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: ru_maxrss is in bytes
        Some(maxrss as u64)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux: ru_maxrss is in kilobytes
        Some(maxrss as u64 * 1024)
    }
}

#[cfg(not(unix))]
fn get_children_peak_rss_bytes() -> Option<u64> {
    None
}

// ============================================================
// Command Resolution (Unix)
// ============================================================

/// Resolve a bare command name (e.g. "python3", "node") to its absolute path via PATH lookup.
#[cfg(unix)]
pub fn resolve_which(cmd: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let which_cmd = "/usr/bin/which";
    #[cfg(not(target_os = "macos"))]
    let which_cmd = "which";

    Command::new(which_cmd)
        .arg(cmd)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
            None
        })
}

/// Resolve a command path: returns as-is if absolute, otherwise looks up via PATH.
#[cfg(unix)]
pub fn resolve_command_path(cmd: &Path) -> PathBuf {
    if cmd.is_absolute() {
        cmd.to_path_buf()
    } else {
        resolve_which(cmd).unwrap_or_else(|| cmd.to_path_buf())
    }
}

// ============================================================
// Resource Limits — pre_exec helpers (Unix)
// ============================================================

/// Apply POSIX resource limits (RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NPROC).
///
/// # Safety
/// Must be called inside a `pre_exec` closure (after fork, before exec).
#[cfg(unix)]
pub unsafe fn apply_rlimits(
    memory_limit_mb: u64,
    cpu_limit_secs: u64,
    file_size_limit_mb: u64,
    max_processes: u64,
) {
    use nix::libc::{rlimit, setrlimit, RLIMIT_AS, RLIMIT_CPU, RLIMIT_FSIZE, RLIMIT_NPROC};

    let memory_limit_bytes = memory_limit_mb * 1024 * 1024;
    let mem = rlimit {
        rlim_cur: memory_limit_bytes,
        rlim_max: memory_limit_bytes,
    };
    setrlimit(RLIMIT_AS, &mem);

    let cpu = rlimit {
        rlim_cur: cpu_limit_secs,
        rlim_max: cpu_limit_secs,
    };
    setrlimit(RLIMIT_CPU, &cpu);

    let file = rlimit {
        rlim_cur: file_size_limit_mb * 1024 * 1024,
        rlim_max: file_size_limit_mb * 1024 * 1024,
    };
    setrlimit(RLIMIT_FSIZE, &file);

    let nproc = rlimit {
        rlim_cur: max_processes,
        rlim_max: max_processes,
    };
    setrlimit(RLIMIT_NPROC, &nproc);
}

/// Register a `pre_exec` hook that applies the standard resource limits.
///
/// # Safety
/// `pre_exec` closures run after fork in the child process.
#[cfg(unix)]
pub unsafe fn set_rlimits_pre_exec(cmd: &mut Command, limits: &ResourceLimits) {
    use std::os::unix::process::CommandExt;

    let memory_limit_mb = limits.max_memory_mb;
    let cpu_limit_secs = limits.timeout_secs;
    let file_size_limit_mb = DEFAULT_FILE_SIZE_LIMIT_MB;
    let max_processes = effective_max_processes();

    cmd.pre_exec(move || {
        apply_rlimits(
            memory_limit_mb,
            cpu_limit_secs,
            file_size_limit_mb,
            max_processes,
        );
        Ok(())
    });
}

// ============================================================
// Network Proxy Setup
// ============================================================

/// Create and start a network proxy if the resolved policy requires domain filtering.
/// Returns `None` if no proxy is needed or if proxy creation/start fails.
pub fn start_network_proxy(network_policy: &ResolvedNetworkPolicy) -> Option<ProxyManager> {
    if security_policy::should_use_proxy(network_policy) {
        let domains = match network_policy {
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
                    crate::info_log!(
                        "[INFO] Network proxy started - HTTP: {:?}, SOCKS5: {:?}",
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
    } else if security_policy::is_allow_all_network(network_policy) {
        crate::info_log!("[INFO] Network access allowed for all domains (wildcard '*' configured)");
        None
    } else {
        None
    }
}

// ============================================================
// Spawn + stdin + wait helpers
// ============================================================

/// Spawn a child, pipe `input_json` to its stdin, and wait with timeout/memory monitoring.
///
/// The command must have `stdin(Stdio::piped())`, `stdout(Stdio::piped())`,
/// `stderr(Stdio::piped())` already set.
///
/// Returns `(ExecutionResult, was_killed, kill_reason)`.
pub fn spawn_write_and_wait(
    cmd: &mut Command,
    input_json: &str,
    limits: &ResourceLimits,
    stream_stderr: bool,
    spawn_context: &str,
) -> Result<(ExecutionResult, bool, Option<String>)> {
    let mut child = cmd.spawn().with_context(|| spawn_context.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .with_context(|| "Failed to write to stdin")?;
    }

    let (stdout, stderr, exit_code, was_killed, kill_reason) = wait_with_timeout(
        &mut child,
        limits.timeout_secs,
        limits.max_memory_bytes(),
        stream_stderr,
    )?;

    Ok((
        ExecutionResult {
            stdout,
            stderr,
            exit_code,
        },
        was_killed,
        kill_reason,
    ))
}

/// Prepare stdin/stdout/stderr piping on a `Command`.
pub fn pipe_stdio(cmd: &mut Command) {
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
}

/// On Windows, avoid allocating a visible console when spawning CLI children from a GUI parent.
#[cfg(target_os = "windows")]
pub fn hide_child_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
pub fn hide_child_console(_cmd: &mut Command) {}

// ============================================================
// Script Arguments Helper
// ============================================================

/// Read script arguments from environment config (SKILLLITE_SCRIPT_ARGS / SKILLBOX_SCRIPT_ARGS).
pub fn get_script_args_from_env() -> Vec<String> {
    if let Some(ref script_args) = skilllite_core::config::SandboxEnvConfig::from_env().script_args
    {
        if !script_args.is_empty() {
            return script_args.split_whitespace().map(String::from).collect();
        }
    }
    Vec::new()
}

// ============================================================
// Shared unsandboxed execution (Unix)
// ============================================================

/// Execute a skill without sandbox isolation on Unix (macOS / Linux).
///
/// This is the shared implementation for the "no sandbox" path. Both macOS and
/// Linux delegate here to avoid duplicating runtime resolution, env setup,
/// resource limits, and process spawning logic.
#[cfg(unix)]
pub fn execute_unsandboxed(
    skill_dir: &Path,
    runtime: &crate::runner::RuntimePaths,
    config: &crate::runner::SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    use crate::runtime_resolver::RuntimeResolver;

    let resolved = runtime.resolve(&config.language).ok_or_else(|| {
        crate::Error::validation(format!("Unsupported language: {}", config.language))
    })?;

    let temp_dir = tempfile::TempDir::new()?;
    let work_dir = temp_dir.path();

    let mut cmd = Command::new(&resolved.interpreter);
    cmd.arg(&config.entry_point);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }
    for arg in get_script_args_from_env() {
        cmd.arg(arg);
    }

    cmd.current_dir(skill_dir);
    pipe_stdio(&mut cmd);

    apply_standard_execution_env(&mut cmd, false, work_dir, config.network_enabled, false);

    unsafe { set_rlimits_pre_exec(&mut cmd, &limits) };

    let (result, _, _) = spawn_write_and_wait(
        &mut cmd,
        input_json,
        &limits,
        true,
        "Failed to spawn skill process",
    )?;
    Ok(result)
}

// ============================================================
// Shared execution env wiring
// ============================================================

/// Apply shared sandbox/runtime environment variables to a spawned command.
///
/// This keeps marker/env compatibility behavior consistent across platforms.
pub fn apply_standard_execution_env(
    cmd: &mut Command,
    sandbox_enabled: bool,
    tmp_dir: &Path,
    network_enabled: bool,
    include_output_dir: bool,
) {
    let sandbox_flag = if sandbox_enabled { "1" } else { "0" };
    cmd.env("SKILLLITE_SANDBOX", sandbox_flag);
    cmd.env("SKILLBOX_SANDBOX", sandbox_flag); // legacy compat
    cmd.env("TMPDIR", tmp_dir);

    #[cfg(windows)]
    {
        cmd.env("TEMP", tmp_dir);
        cmd.env("TMP", tmp_dir);
    }

    if include_output_dir {
        if let Some(ref output_dir) = skilllite_core::config::PathsConfig::from_env().output_dir {
            cmd.env("SKILLLITE_OUTPUT_DIR", output_dir);
        }
    }

    if !network_enabled {
        cmd.env("SKILLLITE_NETWORK_DISABLED", "1");
        cmd.env("SKILLBOX_NETWORK_DISABLED", "1"); // legacy compat
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_memory_check_interval() {
        assert_eq!(MEMORY_CHECK_INTERVAL_MS, 100);
    }

    #[test]
    fn env_compat_prefers_new_key() {
        let _g = ENV_MUTEX.lock().expect("env test lock");
        const K_NEW: &str = "SKILLLITE_UT_ENV_COMPAT_NEW";
        const K_OLD: &str = "SKILLBOX_UT_ENV_COMPAT_OLD";
        std::env::remove_var(K_NEW);
        std::env::remove_var(K_OLD);
        std::env::set_var(K_NEW, "alpha");
        assert_eq!(env_compat(K_NEW, K_OLD).unwrap(), "alpha");
        std::env::remove_var(K_NEW);
        std::env::set_var(K_OLD, "beta");
        assert_eq!(env_compat(K_NEW, K_OLD).unwrap(), "beta");
        std::env::remove_var(K_OLD);
    }

    #[test]
    fn env_compat_is_set_either_key() {
        let _g = ENV_MUTEX.lock().expect("env test lock");
        const K_NEW: &str = "SKILLLITE_UT_ENV_COMPAT_SET_A";
        const K_OLD: &str = "SKILLBOX_UT_ENV_COMPAT_SET_B";
        std::env::remove_var(K_NEW);
        std::env::remove_var(K_OLD);
        assert!(!env_compat_is_set(K_NEW, K_OLD));
        std::env::set_var(K_NEW, "1");
        assert!(env_compat_is_set(K_NEW, K_OLD));
        std::env::remove_var(K_NEW);
        std::env::set_var(K_OLD, "1");
        assert!(env_compat_is_set(K_NEW, K_OLD));
        std::env::remove_var(K_OLD);
    }
}
