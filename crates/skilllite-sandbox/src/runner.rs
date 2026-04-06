use crate::error::bail;
use crate::security::{run_skill_precheck, SKILL_PRECHECK_CRITICAL_BLOCKED};
use crate::Result;
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

/// Optional behavior for [`run_in_sandbox_with_limits_and_level_opt`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SandboxRunOptions {
    /// When `true`, skip the unified skill precheck (`SKILL.md` + entry [`ScriptScanner`]) before spawn.
    /// Use when the caller (e.g. agent or MCP Level 3) already ran [`crate::security::run_skill_precheck`]
    /// with the same policy and obtained consent.
    pub skip_skill_precheck: bool,
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

/// After the precheck report is printed to stderr, prompt on stdin (TTY only). Caller handles
/// `SKILLLITE_AUTO_APPROVE` and non-TTY before invoking this.
fn prompt_skill_precheck_continue(skill_id: &str) -> bool {
    eprintln!();
    eprintln!("── Skill static precheck ──");
    eprintln!("Review the report above. This prompt uses the same policy as SKILLLITE_AUTO_APPROVE / TTY checks.");
    eprintln!();

    loop {
        eprint!("  Continue execution? [y/N]: ");
        let _ = io::stderr().flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("\n  Input error, cancelling");
            observability::audit_confirmation_response(skill_id, false, "user");
            return false;
        }

        let input = input.trim().to_lowercase();
        match input.as_str() {
            "y" | "yes" => {
                eprintln!("  Approved — proceeding.");
                observability::audit_confirmation_response(skill_id, true, "user");
                return true;
            }
            "n" | "no" | "" => {
                eprintln!("  Cancelled by user");
                observability::audit_confirmation_response(skill_id, false, "user");
                return false;
            }
            _ => {
                eprintln!("  Please enter 'y' to continue or 'n' to cancel.");
            }
        }
    }
}

/// Skill precheck gate: `SKILLLITE_AUTO_APPROVE` / non-TTY / stdin consent after stderr report.
fn skill_precheck_enforce_interactive_consent(skill_id: &str, report: &str) -> Result<()> {
    let auto_approve = skilllite_core::config::SandboxEnvConfig::from_env().auto_approve;

    if !auto_approve {
        eprintln!("{}", report);
    }

    observability::audit_confirmation_requested(skill_id, "", 1, "SKILL_STATIC_PRECHECK");
    observability::security_scan_high(
        skill_id,
        "SKILL_STATIC_PRECHECK",
        &serde_json::Value::Array(vec![]),
    );

    let approved = if auto_approve {
        tracing::info!(
            "Auto-approved via SKILLLITE_AUTO_APPROVE (or legacy SKILLBOX_AUTO_APPROVE)"
        );
        observability::audit_confirmation_response(skill_id, true, "auto");
        true
    } else if !io::stdin().is_terminal() {
        tracing::warn!(
            "Non-TTY stdin: blocking skill static precheck (set SKILLLITE_AUTO_APPROVE=1 to override)"
        );
        observability::audit_confirmation_response(skill_id, false, "non-tty-blocked");
        false
    } else {
        prompt_skill_precheck_continue(skill_id)
    };

    if !approved {
        bail!("Script execution blocked: User denied authorization after skill static precheck");
    }
    Ok(())
}

/// Run a skill in a sandboxed environment with custom resource limits and security level.
///
/// Equivalent to [`run_in_sandbox_with_limits_and_level_opt`] with default options (full L3 scan in-runner).
pub fn run_in_sandbox_with_limits_and_level(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
    level: SandboxLevel,
) -> Result<String> {
    run_in_sandbox_with_limits_and_level_opt(
        skill_dir,
        runtime,
        config,
        input_json,
        limits,
        level,
        SandboxRunOptions::default(),
    )
}

/// Run a skill with optional sandbox behavior (e.g. skip duplicate skill precheck when agent pre-gated).
pub fn run_in_sandbox_with_limits_and_level_opt(
    skill_dir: &Path,
    runtime: &RuntimePaths,
    config: &SandboxConfig,
    input_json: &str,
    limits: ResourceLimits,
    level: SandboxLevel,
    options: SandboxRunOptions,
) -> Result<String> {
    tracing::info!(
        level = ?level,
        mode = %match level {
            SandboxLevel::Level1 => "No sandbox - direct execution",
            SandboxLevel::Level2 => "Sandbox isolation only",
            SandboxLevel::Level3 => "Sandbox isolation + static code scanning",
        },
        skip_skill_precheck = options.skip_skill_precheck,
        "Sandbox execution start"
    );

    // Pre-spawn static precheck: SKILL.md + entry script (all levels L1–L3). Skip when the caller
    // already gated (agent desktop, MCP Level 3).
    if !options.skip_skill_precheck {
        let summary = run_skill_precheck(skill_dir, &config.entry_point, config.network_enabled);
        if summary.has_critical_script_issue {
            let tail = summary
                .review_text
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("\n\n{}", s))
                .unwrap_or_default();
            bail!("{}{}", SKILL_PRECHECK_CRITICAL_BLOCKED, tail);
        }
        if let Some(ref report) = summary.review_text {
            skill_precheck_enforce_interactive_consent(&config.name, report)?;
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
            bail!(
                "Skill execution failed with exit code {}: {}",
                result.exit_code,
                result.stderr
            );
        }

        let output = result.stdout.trim();
        let _: serde_json::Value = serde_json::from_str(output).map_err(|e| {
            crate::Error::validation(format!(
                "Skill output is not valid JSON: {} - Output: {}",
                e, output
            ))
        })?;

        observability::audit_execution_completed(
            &config.name,
            result.exit_code,
            start.elapsed().as_millis() as u64,
            result.stdout.len(),
        );
        observability::audit_skill_invocation(
            &config.name,
            &config.entry_point,
            skill_dir.to_string_lossy().as_ref(),
            input_json,
            output,
            result.exit_code,
            start.elapsed().as_millis() as u64,
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
        bail!(
            "Skill execution failed with exit code {}: {}",
            result.exit_code,
            result.stderr
        );
    }

    let output = result.stdout.trim();
    let _: serde_json::Value = serde_json::from_str(output).map_err(|e| {
        crate::Error::validation(format!(
            "Skill output is not valid JSON: {} - Output: {}",
            e, output
        ))
    })?;

    observability::audit_execution_completed(
        &config.name,
        result.exit_code,
        start.elapsed().as_millis() as u64,
        result.stdout.len(),
    );
    observability::audit_skill_invocation(
        &config.name,
        &config.entry_point,
        skill_dir.to_string_lossy().as_ref(),
        input_json,
        output,
        result.exit_code,
        start.elapsed().as_millis() as u64,
    );
    Ok(output.to_string())
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
fn execute_platform_sandbox_with_limits(
    _skill_dir: &Path,
    _runtime: &RuntimePaths,
    _config: &SandboxConfig,
    _input_json: &str,
    _limits: ResourceLimits,
) -> Result<ExecutionResult> {
    bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
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
    return super::linux::execute_simple_with_limits(
        skill_dir, runtime, config, input_json, limits,
    );

    #[cfg(target_os = "windows")]
    return super::windows::execute_simple_with_limits(
        skill_dir, runtime, config, input_json, limits,
    );

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    bail!("Unsupported platform. Only Linux, macOS, and Windows are supported.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_level_from_cli_maps_1_2_3() {
        assert_eq!(SandboxLevel::from_env_or_cli(Some(1)), SandboxLevel::Level1);
        assert_eq!(SandboxLevel::from_env_or_cli(Some(2)), SandboxLevel::Level2);
        assert_eq!(SandboxLevel::from_env_or_cli(Some(3)), SandboxLevel::Level3);
    }

    #[test]
    fn sandbox_level_invalid_cli_defaults_to_level3() {
        assert_eq!(SandboxLevel::from_env_or_cli(Some(0)), SandboxLevel::Level3);
        assert_eq!(SandboxLevel::from_env_or_cli(Some(9)), SandboxLevel::Level3);
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

    #[test]
    fn sandbox_run_options_default_does_not_skip_skill_precheck() {
        let o = SandboxRunOptions::default();
        assert!(!o.skip_skill_precheck);
    }
}
