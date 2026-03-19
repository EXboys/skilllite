use crate::security::{format_scan_result_compact, ScriptScanner, SecuritySeverity};
use anyhow::Result;
use skilllite_core::observability;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxLevel {
    /// Level 1: No sandbox - execute directly without any isolation
    Level1,
    /// Level 2: Sandbox isolation only (macOS Seatbelt / Linux namespace + seccomp)
    Level2,
    /// Level 3: Sandbox isolation + static code scanning (default)
    #[default]
    Level3,
}

impl SandboxLevel {
    /// Parse sandbox level from string or config (CLI overrides env/config)
    pub fn from_env_or_cli(cli_level: Option<u8>) -> Self {
        // Priority: CLI > Config (SKILLLITE_* / SKILLBOX_*) > Default (Level 3)
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
        let cfg = skilllite_core::config::SandboxEnvConfig::from_env();
        match cfg.sandbox_level {
            1 => Self::Level1,
            2 => Self::Level2,
            3 => Self::Level3,
            _ => Self::Level3,
        }
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

    /// Load resource limits from config (SKILLLITE_* / SKILLBOX_* 统一走 config)
    pub fn from_env() -> Self {
        let cfg = skilllite_core::config::SandboxEnvConfig::from_env();
        Self {
            max_memory_mb: cfg.max_memory_mb,
            timeout_secs: cfg.timeout_secs,
        }
    }

    /// Override with CLI parameters
    pub fn with_cli_overrides(
        mut self,
        cli_max_memory: Option<u64>,
        cli_timeout: Option<u64>,
    ) -> Self {
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
    eprintln!(
        "│  Found {} {} severity issue(s) that need your attention.",
        issues_count, severity
    );
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

    // Check if auto-approve is enabled via config (SKILLLITE_* / SKILLBOX_*)
    if skilllite_core::config::SandboxEnvConfig::from_env().auto_approve {
        tracing::info!(
            "Auto-approved via SKILLLITE_AUTO_APPROVE (or legacy SKILLBOX_AUTO_APPROVE)"
        );
        observability::audit_confirmation_response(skill_id, true, "auto");
        return true;
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

            let critical_issues: Vec<_> = scan_result
                .issues
                .iter()
                .filter(|issue| matches!(issue.severity, SecuritySeverity::Critical))
                .collect();
            let high_issues: Vec<_> = scan_result
                .issues
                .iter()
                .filter(|issue| matches!(issue.severity, SecuritySeverity::High))
                .collect();

            if !critical_issues.is_empty() || !high_issues.is_empty() {
                let auto_approve =
                    skilllite_core::config::SandboxEnvConfig::from_env().auto_approve;

                if !auto_approve {
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

                let approved = if auto_approve {
                    tracing::info!(
                        "Auto-approved via SKILLLITE_AUTO_APPROVE (agent/daemon already confirmed)"
                    );
                    observability::audit_confirmation_response(&config.name, true, "auto");
                    true
                } else if !io::stdin().is_terminal() {
                    tracing::warn!(
                        "Non-TTY stdin: blocking {} severity issues (set SKILLLITE_AUTO_APPROVE=1 to override)",
                        severity_str
                    );
                    observability::audit_confirmation_response(
                        &config.name,
                        false,
                        "non-tty-blocked",
                    );
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

            if !scan_result.issues.is_empty()
                && critical_issues.is_empty()
                && high_issues.is_empty()
            {
                eprintln!("{}", format_scan_result_compact(&scan_result));
            }
        }
    }

    // Level 1: Execute without sandbox
    if !level.use_sandbox() {
        tracing::warn!(
            "Running without sandbox (Level 1) - no isolation, but with resource limits"
        );
        observability::audit_command_invoked(
            &config.name,
            &config.entry_point,
            &[],
            skill_dir.to_string_lossy().as_ref(),
        );
        let start = Instant::now();
        let result =
            execute_simple_without_sandbox(skill_dir, runtime, config, input_json, limits)?;

        if result.exit_code != 0 {
            anyhow::bail!(
                "Skill execution failed with exit code {}: {}",
                result.exit_code,
                result.stderr
            );
        }

        let output = result.stdout.trim();
        let _: serde_json::Value = serde_json::from_str(output).map_err(|e| {
            anyhow::anyhow!("Skill output is not valid JSON: {} - Output: {}", e, output)
        })?;

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
    let result =
        execute_platform_sandbox_with_limits(skill_dir, runtime, config, input_json, limits)?;

    if result.exit_code != 0 {
        anyhow::bail!(
            "Skill execution failed with exit code {}: {}",
            result.exit_code,
            result.stderr
        );
    }

    let output = result.stdout.trim();
    let _: serde_json::Value = serde_json::from_str(output).map_err(|e| {
        anyhow::anyhow!("Skill output is not valid JSON: {} - Output: {}", e, output)
    })?;

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
        skill_dir, runtime, config, input_json, limits,
    );

    #[cfg(target_os = "linux")]
    return super::linux::execute_with_limits(skill_dir, runtime, config, input_json, limits);

    #[cfg(target_os = "windows")]
    return super::windows::execute_simple_with_limits(
        skill_dir, runtime, config, input_json, limits,
    );

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    anyhow::bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_level_from_cli_maps_1_2_3() {
        assert_eq!(
            SandboxLevel::from_env_or_cli(Some(1)),
            SandboxLevel::Level1
        );
        assert_eq!(
            SandboxLevel::from_env_or_cli(Some(2)),
            SandboxLevel::Level2
        );
        assert_eq!(
            SandboxLevel::from_env_or_cli(Some(3)),
            SandboxLevel::Level3
        );
    }

    #[test]
    fn sandbox_level_invalid_cli_defaults_to_level3() {
        assert_eq!(
            SandboxLevel::from_env_or_cli(Some(0)),
            SandboxLevel::Level3
        );
        assert_eq!(
            SandboxLevel::from_env_or_cli(Some(9)),
            SandboxLevel::Level3
        );
    }

    #[test]
    fn sandbox_level_use_flags() {
        assert!(!SandboxLevel::Level1.use_sandbox());
        assert!(!SandboxLevel::Level1.use_code_scanning());
        assert!(SandboxLevel::Level2.use_sandbox());
        assert!(!SandboxLevel::Level2.use_code_scanning());
        assert!(SandboxLevel::Level3.use_sandbox());
        assert!(SandboxLevel::Level3.use_code_scanning());
    }

    #[test]
    fn resource_limits_max_memory_bytes() {
        let lim = ResourceLimits {
            max_memory_mb: 128,
            timeout_secs: 10,
        };
        assert_eq!(lim.max_memory_bytes(), 128 * 1024 * 1024);
    }

    #[test]
    fn resource_limits_with_cli_overrides() {
        let base = ResourceLimits {
            max_memory_mb: 100,
            timeout_secs: 20,
        };
        let o = base.with_cli_overrides(Some(512), Some(60));
        assert_eq!(o.max_memory_mb, 512);
        assert_eq!(o.timeout_secs, 60);
        let partial = base.with_cli_overrides(None, Some(99));
        assert_eq!(partial.max_memory_mb, 100);
        assert_eq!(partial.timeout_secs, 99);
    }
}
