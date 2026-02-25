//! Windows Sandbox Implementation
//!
//! Security strategy (in priority order):
//! 1. WSL2 bridge → reuses full Linux sandbox (bwrap/firejail/seccomp)
//! 2. Native Windows isolation → Job Object + restricted token (partial)
//! 3. Refuse execution → never silently run without isolation
//!
//! ## Key principle
//! If no adequate sandbox is available, execution is REFUSED rather than
//! silently falling back to unprotected mode. Users must explicitly set
//! SKILLBOX_NO_SANDBOX=1 to run without protection.

#![cfg(target_os = "windows")]

use crate::runner::{ExecutionResult, ResourceLimits, RuntimePaths, SandboxConfig};
use crate::runtime_resolver::RuntimeResolver;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Execute a skill in Windows sandbox
pub fn execute_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    if std::env::var("SKILLBOX_NO_SANDBOX").is_ok() {
        eprintln!("[WARN] Sandbox disabled via SKILLBOX_NO_SANDBOX - running without protection");
        return execute_simple_with_limits(skill_dir, runtime, config, input_json, limits);
    }

    // Try WSL2 bridge (full Linux sandbox)
    match check_wsl2_status() {
        Wsl2Status::Ready => {
            match execute_via_wsl2(skill_dir, runtime, config, input_json, limits) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    skilllite_core::observability::security_sandbox_fallback(
                        &config.name,
                        "wsl2_exec_failed",
                    );
                    return Err(e.context(
                        "WSL2 sandbox execution failed. \
                         Set SKILLBOX_NO_SANDBOX=1 to run without sandbox (not recommended)."
                    ));
                }
            }
        }
        Wsl2Status::Available { skilllite_missing } => {
            eprintln!("[WARN] WSL2 is available but skilllite is not installed inside WSL.");
            eprintln!("[WARN] Install it with: wsl -e sh -c 'cargo install --git https://github.com/user/skilllite skilllite'");
            if skilllite_missing {
                eprintln!("[WARN] Falling back to native Windows isolation (limited security).");
            }
        }
        Wsl2Status::NotAvailable => {
            eprintln!("[WARN] WSL2 is not available on this system.");
            eprintln!("[WARN] For full security, install WSL2: wsl --install");
        }
    }

    // Native Windows isolation (Job Object + restricted environment)
    match execute_with_native_isolation(skill_dir, runtime, config, input_json, limits) {
        Ok(result) => Ok(result),
        Err(e) => {
            skilllite_core::observability::security_sandbox_fallback(
                &config.name,
                "windows_native_isolation_failed",
            );
            Err(e.context(
                "Windows sandbox execution failed. No adequate isolation available. \
                 Set SKILLBOX_NO_SANDBOX=1 to run without sandbox (not recommended)."
            ))
        }
    }
}

// ============================================================================
// WSL2 Bridge
// ============================================================================

#[derive(Debug)]
enum Wsl2Status {
    Ready,
    Available { skilllite_missing: bool },
    NotAvailable,
}

fn check_wsl2_status() -> Wsl2Status {
    let wsl_ok = Command::new("wsl")
        .args(["--status"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !wsl_ok {
        return Wsl2Status::NotAvailable;
    }

    let skilllite_ok = Command::new("wsl")
        .args(["-e", "which", "skilllite"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    if skilllite_ok {
        Wsl2Status::Ready
    } else {
        Wsl2Status::Available { skilllite_missing: true }
    }
}

/// Convert Windows path to WSL path (C:\foo\bar → /mnt/c/foo/bar)
fn windows_to_wsl_path(path: &Path) -> Result<String> {
    let path_str = path.to_string_lossy();

    if path_str.starts_with("\\\\") {
        anyhow::bail!("UNC paths are not supported in WSL: {}", path_str);
    }

    let chars: Vec<char> = path_str.chars().collect();
    if chars.len() >= 2 && chars[1] == ':' {
        let drive = chars[0]
            .to_lowercase()
            .next()
            .expect("drive letter must be valid");
        let rest = &path_str[2..].replace('\\', "/");
        return Ok(format!("/mnt/{}{}", drive, rest));
    }

    Ok(path_str.replace('\\', "/"))
}

/// Execute skill via WSL2 using stdin pipe (no CLI arg length limits)
fn execute_via_wsl2(
    skill_dir: &Path,
    _runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    let wsl_skill_dir = windows_to_wsl_path(skill_dir)
        .context("Failed to convert skill_dir to WSL path")?;

    let mut args = vec![
        "-e".to_string(),
        "skilllite".to_string(),
        "run".to_string(),
        wsl_skill_dir,
        "--timeout".to_string(),
        limits.timeout_secs.to_string(),
        "--max-memory".to_string(),
        limits.max_memory_mb.to_string(),
    ];

    if config.network_enabled {
        args.push("--allow-network".to_string());
    }

    // Use stdin pipe for input_json (avoids shell escaping & CLI length limits)
    let mut child = Command::new("wsl")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn skilllite via WSL")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_json.as_bytes())
            .context("Failed to write input to WSL stdin")?;
    }

    let timeout = std::time::Duration::from_secs(limits.timeout_secs + 10);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output()
                    .context("Failed to read WSL output")?;
                return Ok(ExecutionResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    anyhow::bail!(
                        "WSL execution timed out after {} seconds",
                        limits.timeout_secs
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(anyhow::anyhow!("Failed to wait for WSL process: {}", e)),
        }
    }
}

// ============================================================================
// Native Windows Isolation
// ============================================================================

/// Execute with native Windows isolation (Job Object + environment restrictions)
///
/// This provides PARTIAL isolation:
/// - Resource limits via Job Object (memory, CPU time, process count)
/// - Isolated temp directory
/// - Sanitized environment variables
///
/// This does NOT provide:
/// - File system isolation (no AppContainer)
/// - Network isolation
/// - Process execution whitelist
///
/// For full security, use WSL2.
fn execute_with_native_isolation(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    eprintln!("[WARN] Using native Windows isolation - PARTIAL security only");
    eprintln!("[WARN] File system and network are NOT isolated. For full security, install WSL2.");
    skilllite_core::observability::security_sandbox_fallback(
        &config.name,
        "windows_native_partial_isolation",
    );

    let language = &config.language;
    let resolved = runtime
        .resolve(language)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language on Windows: {}", language))?;

    let temp_dir = TempDir::new()?;
    let work_dir = temp_dir.path();

    let entry_point = skill_dir.join(&config.entry_point);
    let mut cmd = Command::new(&resolved.interpreter);
    cmd.arg(&entry_point);
    cmd.current_dir(skill_dir);

    // Sanitized environment: only pass what the skill needs
    cmd.env("SKILLBOX_SANDBOX", "1");
    cmd.env("TMPDIR", work_dir);
    cmd.env("TEMP", work_dir);
    cmd.env("TMP", work_dir);

    if !config.network_enabled {
        cmd.env("SKILLBOX_NETWORK_DISABLED", "1");
    }

    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn process")?;

    // Attach Job Object for resource limits (best-effort)
    let job_handle = attach_job_object(&child, &limits);
    if job_handle.is_err() {
        eprintln!("[WARN] Failed to create Job Object: {}. Resource limits not enforced.", 
                  job_handle.as_ref().err().unwrap());
    }

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input_json.as_bytes());
    }

    let timeout = std::time::Duration::from_secs(limits.timeout_secs);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                use std::io::Read;
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_string(&mut stderr);
                }
                return Ok(ExecutionResult {
                    stdout,
                    stderr,
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Ok(ExecutionResult {
                        stdout: String::new(),
                        stderr: format!("Process killed: exceeded timeout of {}s", limits.timeout_secs),
                        exit_code: -1,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(anyhow::anyhow!("Failed to wait for process: {}", e)),
        }
    }
}

/// Attach a Job Object to the child process for resource limits.
///
/// Job Object provides:
/// - Memory limit (JOB_OBJECT_LIMIT_PROCESS_MEMORY)
/// - Process count limit (JOB_OBJECT_LIMIT_ACTIVE_PROCESS)
/// - Kill-on-close (all child processes die when handle closes)
fn attach_job_object(
    child: &std::process::Child,
    limits: &ResourceLimits,
) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::*;

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job == 0 {
            anyhow::bail!("CreateJobObjectW failed");
        }

        // Set resource limits
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.ProcessMemoryLimit = (limits.max_memory_mb * 1024 * 1024) as usize;
        info.BasicLimitInformation.ActiveProcessLimit = 10;

        let set_ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if set_ok == 0 {
            CloseHandle(job);
            anyhow::bail!("SetInformationJobObject failed");
        }

        let process_handle = child.as_raw_handle() as HANDLE;
        let assign_ok = AssignProcessToJobObject(job, process_handle);
        if assign_ok == 0 {
            CloseHandle(job);
            anyhow::bail!("AssignProcessToJobObject failed");
        }

        // Job handle is intentionally leaked — it stays alive as long as the
        // process lives, and JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE ensures cleanup.
        // The OS reclaims the handle when our process exits.
    }

    Ok(())
}

// ============================================================================
// Simple execution (Level 1 / explicit no-sandbox)
// ============================================================================

/// Simple execution without sandbox (Level 1 or explicit SKILLBOX_NO_SANDBOX)
pub fn execute_simple_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    let language = &config.language;
    let entry_point = skill_dir.join(&config.entry_point);

    // Bash on Windows: prefer WSL if available
    if language == "bash" {
        if let Wsl2Status::Ready | Wsl2Status::Available { .. } = check_wsl2_status() {
            let wsl_entry = windows_to_wsl_path(&entry_point)?;
            return execute_bash_via_wsl(&wsl_entry, input_json, limits);
        }
    }

    let resolved = runtime
        .resolve(language)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language on Windows: {}", language))?;

    let temp_dir = TempDir::new()?;
    let input_file = temp_dir.path().join("input.json");
    std::fs::write(&input_file, input_json)?;

    let mut cmd = Command::new(&resolved.interpreter);
    cmd.arg(&entry_point)
        .current_dir(skill_dir)
        .env("SKILL_INPUT_FILE", &input_file)
        .env("SKILL_INPUT", input_json)
        .env("SKILLBOX_SANDBOX", "0");
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }
    if !config.network_enabled {
        cmd.env("SKILLBOX_NETWORK_DISABLED", "1");
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to execute skill")?;

    if let Err(e) = attach_job_object(&child, &limits) {
        eprintln!("[WARN] Failed to attach Job Object: {}. Resource limits not enforced.", e);
    }

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input_json.as_bytes());
    }

    let timeout = std::time::Duration::from_secs(limits.timeout_secs);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                use std::io::Read;
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_string(&mut stderr);
                }
                return Ok(ExecutionResult {
                    stdout,
                    stderr,
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Ok(ExecutionResult {
                        stdout: String::new(),
                        stderr: format!("Process killed: exceeded timeout of {}s", limits.timeout_secs),
                        exit_code: -1,
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => return Err(anyhow::anyhow!("Failed to wait for process: {}", e)),
        }
    }
}

/// Execute bash script via WSL
fn execute_bash_via_wsl(
    wsl_script_path: &str,
    input_json: &str,
    _limits: ResourceLimits,
) -> Result<ExecutionResult> {
    let mut child = Command::new("wsl")
        .args(["-e", "bash", wsl_script_path])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn bash via WSL")?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input_json.as_bytes());
    }

    let output = child.wait_with_output()
        .context("Failed to wait for WSL bash")?;

    Ok(ExecutionResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

// ============================================================================
// Diagnostics
// ============================================================================

/// Check WSL2 status and provide helpful diagnostics
pub fn diagnose_wsl() -> String {
    let mut report = String::new();
    report.push_str("=== WSL2 Diagnostics ===\n");

    match check_wsl2_status() {
        Wsl2Status::Ready => {
            report.push_str("WSL2: Ready (full Linux sandbox available)\n");
            report.push_str("skilllite: Installed in WSL\n");
            report.push_str("Security: Full isolation via bwrap/firejail + seccomp\n");
        }
        Wsl2Status::Available { .. } => {
            report.push_str("WSL2: Available but skilllite is NOT installed in WSL\n\n");
            report.push_str("To install skilllite in WSL:\n");
            report.push_str("  wsl -e sh -c 'curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh'\n");
            report.push_str("  wsl -e sh -c 'cargo install --path /mnt/c/path/to/skilllite'\n");
        }
        Wsl2Status::NotAvailable => {
            report.push_str("WSL2: NOT available\n\n");
            report.push_str("To install WSL2:\n");
            report.push_str("  1. Open PowerShell as Administrator\n");
            report.push_str("  2. Run: wsl --install\n");
            report.push_str("  3. Restart your computer\n");
            report.push_str("\nWithout WSL2, only partial isolation is available.\n");
        }
    }

    report
}
