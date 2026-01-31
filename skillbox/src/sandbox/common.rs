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
pub const DEFAULT_MAX_MEMORY_MB: u64 = 512;

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

/// Wait for child process with timeout and memory monitoring
/// 
/// This function monitors a child process and enforces resource limits:
/// - Timeout: kills the process if it exceeds the specified duration
/// - Memory limit: kills the process if RSS exceeds the specified bytes
/// 
/// # Arguments
/// * `child` - The child process to monitor
/// * `timeout_secs` - Maximum execution time in seconds
/// * `memory_limit_bytes` - Maximum memory usage in bytes
/// 
/// # Returns
/// A tuple of (stdout, stderr, exit_code, was_killed, kill_reason)
/// - `was_killed`: true if the process was killed due to resource limits
/// - `kill_reason`: "timeout" or "memory_limit" if killed, None otherwise
pub fn wait_with_timeout(
    child: &mut Child,
    timeout_secs: u64,
    memory_limit_bytes: u64,
) -> Result<(String, String, i32, bool, Option<String>)> {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let check_interval = Duration::from_millis(MEMORY_CHECK_INTERVAL_MS);
    
    loop {
        // Check if process has exited
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited normally
                let mut stdout = String::new();
                let mut stderr = String::new();
                
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_string(&mut stderr);
                }
                
                return Ok((stdout, stderr, status.code().unwrap_or(-1), false, None));
            }
            Ok(None) => {
                // Process still running, check constraints
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to wait for process: {}", e));
            }
        }
        
        // Check timeout
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((
                String::new(),
                format!("Process killed: exceeded timeout of {} seconds", timeout_secs),
                -1,
                true,
                Some("timeout".to_string()),
            ));
        }
        
        // Check memory usage
        if let Some(memory) = get_process_memory(child.id()) {
            if memory > memory_limit_bytes {
                let _ = child.kill();
                let _ = child.wait();
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
        
        // Sleep before next check
        thread::sleep(check_interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_memory_check_interval() {
        assert_eq!(MEMORY_CHECK_INTERVAL_MS, 100);
    }
}
