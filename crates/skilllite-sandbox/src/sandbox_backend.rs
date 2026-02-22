//! SandboxBackend trait: extension point for sandbox implementations.
//!
//! Implement this trait to add new sandbox backends (e.g. Landlock on Linux 5.13+).
//! The default backend is selected by platform: Seatbelt (macOS), bwrap/seccomp (Linux),
//! WSL2/Job Object (Windows).

use anyhow::Result;
use std::path::Path;

use crate::runner::{ExecutionResult, ResourceLimits, RuntimePaths, SandboxConfig};

/// Extension point for sandbox execution backends.
///
/// Implement this trait to add new isolation strategies (e.g. Landlock).
/// Selection is typically by platform and/or feature flag.
pub trait SandboxBackend: Send + Sync {
    /// Backend name for logging and diagnostics.
    fn name(&self) -> &str;

    /// Execute a skill in the sandbox with the given configuration.
    fn execute(
        &self,
        skill_dir: &Path,
        runtime: &RuntimePaths,
        config: &SandboxConfig,
        input_json: &str,
        limits: ResourceLimits,
    ) -> Result<ExecutionResult>;
}

/// Default platform-specific sandbox backend.
///
/// - macOS: Seatbelt (sandbox-exec)
/// - Linux: bwrap / firejail / seccomp / namespaces
/// - Windows: WSL2 bridge or Job Object fallback
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSandboxBackend;

impl SandboxBackend for NativeSandboxBackend {
    fn name(&self) -> &str {
        #[cfg(target_os = "macos")]
        return "seatbelt";
        #[cfg(target_os = "linux")]
        return "bwrap/seccomp";
        #[cfg(target_os = "windows")]
        return "wsl2/job";
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return "unsupported";
    }

    fn execute(
        &self,
        skill_dir: &Path,
        runtime: &RuntimePaths,
        config: &SandboxConfig,
        input_json: &str,
        limits: ResourceLimits,
    ) -> Result<ExecutionResult> {
        #[cfg(target_os = "macos")]
        return crate::macos::execute_with_limits(skill_dir, runtime, config, input_json, limits);
        #[cfg(target_os = "linux")]
        return crate::linux::execute_with_limits(skill_dir, runtime, config, input_json, limits);
        #[cfg(target_os = "windows")]
        return crate::windows::execute_with_limits(skill_dir, runtime, config, input_json, limits);
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        anyhow::bail!("Unsupported platform")
    }
}
