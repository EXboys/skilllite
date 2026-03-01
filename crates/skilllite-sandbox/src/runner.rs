use skilllite_core::observability;
use crate::common::{DEFAULT_MAX_MEMORY_MB, DEFAULT_TIMEOUT_SECS};
use crate::security::{format_scan_result_compact, ScriptScanner, SecuritySeverity};
use anyhow::Result;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::time::Instant;

/// Execution result from sandbox
#[derive(Debug)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Resolved runtime paths for sandbox execution.
///
/// Callers construct this via `env::builder` helpers; the sandbox module
/// never imports `env::builder` directly.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    /// Path to the Python interpreter (venv or system `python3`)
    pub python: std::path::PathBuf,
    /// Path to the Node.js interpreter (typically system `node`)
    pub node: std::path::PathBuf,
    /// Path to cached `node_modules` directory, if any
    pub node_modules: Option<std::path::PathBuf>,
    /// Environment directory (Python venv / Node env cache).
    /// Empty `PathBuf` means no isolated environment.
    pub env_dir: std::path::PathBuf,
}

/// Sandbox execution configuration.
///
/// Callers construct this from `SkillMetadata` (or other sources);
/// the sandbox module never imports `skill::metadata` directly.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Skill / task name (used for logging and audit)
    pub name: String,
    /// Entry point script path relative to skill directory
    pub entry_point: String,
    /// Resolved language: "python", "node", or "bash"
    pub language: String,
    /// Whether outbound network access is permitted
    pub network_enabled: bool,
    /// Allowed outbound hosts (e.g. ["*"] for wildcard)
    pub network_outbound: Vec<String>,
    /// Whether the skill uses Playwright (requires relaxed sandbox on macOS)
    pub uses_playwright: bool,
}

/// Sandbox security levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxLevel {
    /// Level 1: No sandbox - execute directly without any isolation
    Level1,
    /// Level 2: Sandbox isolation only (macOS Seatbelt / Linux namespace + seccomp)
    Level2,
    /// Level 3: Sandbox isolation + static code scanning (default)
    Level3,
}

impl Default for SandboxLevel {
    fn default() -> Self {
        Self::Level3
    }
}

impl SandboxLevel {
    /// Parse sandbox level from string or environment variable
    pub fn from_env_or_cli(cli_level: Option<u8>) -> Self {
        // Priority: CLI > Environment Variable > Default (Level 3)
        if let Some(level) = cli_level {
            return match level {
                1 => Self::Level1,
                2 => Self::Level2,
                3 => Self::Level3,
                _ => {
                    tracing::warn!("Invalid sandbox level: {}, using default (3)", level);
                    Self::Level3
                }
            };
        }

        // Read from environment variable
        if let Ok(level_str) = crate::common::env_compat("SKILLLITE_SANDBOX_LEVEL", "SKILLBOX_SANDBOX_LEVEL") {
            if let Ok(level) = level_str.parse::<u8>() {
                return match level {
                    1 => Self::Level1,
                    2 => Self::Level2,
                    3 => Self::Level3,
                    _ => {
                        tracing::warn!(
                            "Invalid SKILLLITE_SANDBOX_LEVEL: {}, using default (3)",
                            level
                        );
                        Self::Level3
                    }
                };
            }
        }

        // Default to Level 3
        Self::Level3
    }

    /// Check if sandbox should be used
    pub fn use_sandbox(&self) -> bool {
        !matches!(self, Self::Level1)
    }

    /// Check if code scanning should be used
    pub fn use_code_scanning(&self) -> bool {
        matches!(self, Self::Level3)
    }
}

/// Resource limits for skill execution
/// 
/// Default values are defined in `common.rs`:
/// - `max_memory_mb`: DEFAULT_MAX_MEMORY_MB (256 MB)
/// - `timeout_secs`: DEFAULT_TIMEOUT_SECS (30 seconds)
#[derive(Debug, Clone, Copy)]
pub struct ResourceLimits {
    /// Maximum memory limit in MB (default: 256)
    pub max_memory_mb: u64,
    /// Execution timeout in seconds (default: 30)
    pub timeout_secs: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::from_env()
    }
}

impl ResourceLimits {
    /// Get memory limit in bytes
    pub fn max_memory_bytes(&self) -> u64 {
        self.max_memory_mb * 1024 * 1024
    }

    /// Load resource limits from environment variables
    pub fn from_env() -> Self {
        let max_memory_mb = crate::common::env_compat("SKILLLITE_MAX_MEMORY_MB", "SKILLBOX_MAX_MEMORY_MB")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_MAX_MEMORY_MB);

        let timeout_secs = crate::common::env_compat("SKILLLITE_TIMEOUT_SECS", "SKILLBOX_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        Self {
            max_memory_mb,
            timeout_secs,
        }
    }

    /// Override with CLI parameters
    pub fn with_cli_overrides(mut self, cli_max_memory: Option<u64>, cli_timeout: Option<u64>) -> Self {
        if let Some(max_memory) = cli_max_memory {
            self.max_memory_mb = max_memory;
        }
        if let Some(timeout) = cli_timeout {
            self.timeout_secs = timeout;
        }
        self
    }
}

/// Request user authorization to continue execution despite security issues
/// Returns true if user authorizes, false otherwise
fn request_user_authorization(skill_id: &str, issues_count: usize, severity: &str) -> bool {
    eprintln!();
    eprintln!("┌─────────────────────────────────────────────────────────────┐");
    eprintln!("│  🔐 Security Review Required                                │");
    eprintln!("├─────────────────────────────────────────────────────────────┤");
    eprintln!("│                                                             │");
    eprintln!("│  Found {} {} severity issue(s) that need your attention.", issues_count, severity);
    eprintln!("│                                                             │");
    eprintln!("│  These operations are flagged for review:                   │");
    eprintln!("│    • System module imports or file access                   │");
    eprintln!("│    • Environment variable access                            │");
    eprintln!("│    • Network requests or external connections               │");
    eprintln!("│    • Process execution commands                             │");
    eprintln!("│                                                             │");
    eprintln!("│  💡 This is a security prompt, not an error.                │");
    eprintln!("│     If you trust this script, you can proceed safely.       │");
    eprintln!("│                                                             │");
    eprintln!("└─────────────────────────────────────────────────────────────┘");
    eprintln!();

    // Check if auto-approve is enabled via environment variable
    if let Ok(val) = crate::common::env_compat("SKILLLITE_AUTO_APPROVE", "SKILLBOX_AUTO_APPROVE") {
        let val_lower = val.to_lowercase();
        if val_lower == "1" || val_lower == "true" || val_lower == "yes" {
            tracing::info!("Auto-approved via SKILLLITE_AUTO_APPROVE={}", val);
            observability::audit_confirmation_response(skill_id, true, "auto");
            return true;
        }
    }
    
    loop {
        eprint!("  👉 Continue execution? [y/N]: ");
        let _ = io::stderr().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("\n  ⏹️  Input error, cancelling");
            return false;
        }
        
        let input = input.trim().to_lowercase();
        match input.as_str() {
            "y" | "yes" => {
                eprintln!();
                eprintln!("  ✅ Approved - proceeding with execution...");
                eprintln!();
                observability::audit_confirmation_response(skill_id, true, "user");
                return true;
            }
            "n" | "no" | "" => {
                eprintln!();
                eprintln!("  ⏹️  Cancelled by user");
                eprintln!();
                observability::audit_confirmation_response(skill_id, false, "user");
                return false;
            }
            _ => {
                eprintln!("  ⚠️  Please enter 'y' to continue or 'n' to cancel.");
            }
        }
    }
}


/// Run a skill in a sandboxed environment with custom resource limits and security level
pub fn run_in_sandbox_with_limits_and_level(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
    level: SandboxLevel,
) -> Result<String> {
    tracing::info!(
        level = ?level,
        mode = %match level {
            SandboxLevel::Level1 => "No sandbox - direct execution",
            SandboxLevel::Level2 => "Sandbox isolation only",
            SandboxLevel::Level3 => "Sandbox isolation + static code scanning",
        },
        "Sandbox execution start"
    );

    // Level 3: Perform static code scanning
    if level.use_code_scanning() {
        let script_path = skill_dir.join(&config.entry_point);
        if script_path.exists() {
            let scanner = ScriptScanner::new()
                .allow_network(config.network_enabled)
                .allow_file_ops(false)
                .allow_process_exec(false);
            
            let scan_result = scanner.scan_file(&script_path)?;
            
            let critical_issues: Vec<_> = scan_result.issues.iter()
                .filter(|issue| matches!(issue.severity, SecuritySeverity::Critical))
                .collect();
            let high_issues: Vec<_> = scan_result.issues.iter()
                .filter(|issue| matches!(issue.severity, SecuritySeverity::High))
                .collect();
            
            if !critical_issues.is_empty() || !high_issues.is_empty() {
                let will_auto_approve = crate::common::env_compat("SKILLLITE_AUTO_APPROVE", "SKILLBOX_AUTO_APPROVE").is_ok_and(|v| {
                    let v = v.trim().to_lowercase();
                    v == "1" || v == "true" || v == "yes"
                });
                if !will_auto_approve {
                    eprintln!("{}", format_scan_result_compact(&scan_result));
                }

                let severity_str = if !critical_issues.is_empty() {
                    "CRITICAL"
                } else {
                    "HIGH"
                };

                let issues_count = critical_issues.len() + high_issues.len();

                let code_hash = "";
                observability::audit_confirmation_requested(
                    &config.name,
                    code_hash,
                    issues_count,
                    severity_str,
                );
                let issues_json: Vec<serde_json::Value> = scan_result
                    .issues
                    .iter()
                    .map(|i| {
                        serde_json::json!({
                            "rule_id": i.rule_id,
                            "line_number": i.line_number,
                            "code_snippet": i.code_snippet,
                            "description": i.description,
                        })
                    })
                    .collect();
                observability::security_scan_high(
                    &config.name,
                    severity_str,
                    &serde_json::Value::Array(issues_json),
                );

                let auto_approve_env = crate::common::env_compat("SKILLLITE_AUTO_APPROVE", "SKILLBOX_AUTO_APPROVE").is_ok_and(|v| {
                    let v = v.trim().to_lowercase();
                    v == "1" || v == "true" || v == "yes"
                });

                let approved = if auto_approve_env {
                    tracing::info!("Auto-approved via SKILLLITE_AUTO_APPROVE (agent/daemon already confirmed)");
                    observability::audit_confirmation_response(&config.name, true, "auto");
                    true
                } else if !io::stdin().is_terminal() {
                    tracing::warn!(
                        "Non-TTY stdin: blocking {} severity issues (set SKILLLITE_AUTO_APPROVE=1 to override)",
                        severity_str
                    );
                    observability::audit_confirmation_response(&config.name, false, "non-tty-blocked");
                    false
                } else {
                    request_user_authorization(&config.name, issues_count, severity_str)
                };

                if !approved {
                    anyhow::bail!(
                        "Script execution blocked: User denied authorization for {} severity issues",
                        severity_str
                    );
                }
            }
            
            if !scan_result.issues.is_empty() && critical_issues.is_empty() && high_issues.is_empty() {
                eprintln!("{}", format_scan_result_compact(&scan_result));
            }
        }
    }

    // Level 1: Execute without sandbox
    if !level.use_sandbox() {
        tracing::warn!("Running without sandbox (Level 1) - no isolation, but with resource limits");
        observability::audit_command_invoked(
            &config.name,
            &config.entry_point,
            &[],
            skill_dir.to_string_lossy().as_ref(),
        );
        let start = Instant::now();
        let result = execute_simple_without_sandbox(skill_dir, runtime, config, input_json, limits)?;
        
        if result.exit_code != 0 {
            anyhow::bail!(
                "Skill execution failed with exit code {}: {}",
                result.exit_code,
                result.stderr
            );
        }

        let output = result.stdout.trim();
        let _: serde_json::Value = serde_json::from_str(output)
            .map_err(|e| anyhow::anyhow!("Skill output is not valid JSON: {} - Output: {}", e, output))?;

        observability::audit_execution_completed(
            &config.name,
            result.exit_code,
            start.elapsed().as_millis() as u64,
            result.stdout.len(),
        );
        return Ok(output.to_string());
    }

    // Level 2 & 3: Execute with sandbox
    observability::audit_command_invoked(
        &config.name,
        &config.entry_point,
        &[] as &[&str],
        skill_dir.to_string_lossy().as_ref(),
    );
    let start = Instant::now();
    let result = execute_platform_sandbox_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        limits,
    )?;

    if result.exit_code != 0 {
        anyhow::bail!(
            "Skill execution failed with exit code {}: {}",
            result.exit_code,
            result.stderr
        );
    }

    let output = result.stdout.trim();
    let _: serde_json::Value = serde_json::from_str(output)
        .map_err(|e| anyhow::anyhow!("Skill output is not valid JSON: {} - Output: {}", e, output))?;

    observability::audit_execution_completed(
        &config.name,
        result.exit_code,
        start.elapsed().as_millis() as u64,
        result.stdout.len(),
    );
    Ok(output.to_string())
}

/// Platform-specific sandbox execution
#[cfg(target_os = "linux")]
fn execute_platform_sandbox(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
) -> Result<ExecutionResult> {
    execute_platform_sandbox_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        ResourceLimits::default(),
    )
}

#[cfg(target_os = "linux")]
fn execute_platform_sandbox_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    super::linux::execute_with_limits(skill_dir, runtime, config, input_json, limits)
}


#[cfg(target_os = "macos")]
fn execute_platform_sandbox_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    super::macos::execute_with_limits(skill_dir, runtime, config, input_json, limits)
}

#[cfg(target_os = "windows")]
fn execute_platform_sandbox(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
) -> Result<ExecutionResult> {
    execute_platform_sandbox_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        ResourceLimits::default(),
    )
}

#[cfg(target_os = "windows")]
fn execute_platform_sandbox_with_limits(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    super::windows::execute_with_limits(skill_dir, runtime, config, input_json, limits)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn execute_platform_sandbox(
    _skill_dir: &Path,
    _runtime: &RuntimePaths,
    _config: &SandboxConfig,
    _input_json: &str,
) -> Result<ExecutionResult> {
    anyhow::bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn execute_platform_sandbox_with_limits(
    _skill_dir: &Path,
    _runtime: &RuntimePaths,
    _config: &SandboxConfig,
    _input_json: &str,
    _limits: ResourceLimits,
) -> Result<ExecutionResult> {
    anyhow::bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
}

/// Execute without any sandbox (Level 1)
fn execute_simple_without_sandbox(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult> {
    #[cfg(target_os = "macos")]
    return super::macos::execute_simple_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        limits,
    );

    #[cfg(target_os = "linux")]
    return super::linux::execute_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        limits,
    );

    #[cfg(target_os = "windows")]
    return super::windows::execute_simple_with_limits(
        skill_dir,
        runtime,
        config,
        input_json,
        limits,
    );

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    anyhow::bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
}
