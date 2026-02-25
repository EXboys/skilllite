//! Common utilities for sandbox implementations
//!
//! This module provides shared functionality used by both macOS and Linux
//! sandbox implementations, including process monitoring and resource limits.

use anyhow::Result;
use std::io::Read;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================
// Resource Limits Constants (Single Source of Truth)
// ============================================================

/// Default maximum memory limit in MB
pub const DEFAULT_MAX_MEMORY_MB: u64 = 256;

/// Default execution timeout in seconds
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default file size limit in MB
pub const DEFAULT_FILE_SIZE_LIMIT_MB: u64 = 10;

/// Maximum number of processes (fork bomb protection)
pub const DEFAULT_MAX_PROCESSES: u64 = 50;

/// Memory check interval in milliseconds
pub const MEMORY_CHECK_INTERVAL_MS: u64 = 100;

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
    let output = Command::new("tasklist")
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
                return Err(anyhow::anyhow!("Failed to wait for process: {}", e));
            }
        }

        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_handle.map(|h| h.join());
            let _ = stderr_handle.map(|h| h.join());
            return Ok((
                String::new(),
                format!("Process killed: exceeded timeout of {} seconds", timeout_secs),
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_check_interval() {
        assert_eq!(MEMORY_CHECK_INTERVAL_MS, 100);
    }
}
