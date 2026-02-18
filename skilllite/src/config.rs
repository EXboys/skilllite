//! Configuration for SkillLite
//!
//! All configuration is read from environment variables or CLI arguments.
//! No global configuration file is used.
//!
//! Environment variable keys are centralized here for consistency.
//! When reading, prefer SKILLLITE_* and fallback to SKILLBOX_* for backward compatibility.

/// Environment variable key constants.
/// Use these when reading/writing env vars to avoid typos and enable refactoring.
#[allow(dead_code)]
pub mod env_keys {
    // ─── SkillLite (primary) ─────────────────────────────────────────────────
    pub const SKILLLITE_CACHE_DIR: &str = "SKILLLITE_CACHE_DIR";
    pub const SKILLLITE_OUTPUT_DIR: &str = "SKILLLITE_OUTPUT_DIR";
    pub const SKILLLITE_SKILLS_DIR: &str = "SKILLLITE_SKILLS_DIR";
    pub const SKILLLITE_MODEL: &str = "SKILLLITE_MODEL";
    pub const SKILLLITE_QUIET: &str = "SKILLLITE_QUIET";
    pub const SKILLLITE_LOG_LEVEL: &str = "SKILLLITE_LOG_LEVEL";
    pub const SKILLLITE_ENABLE_TASK_PLANNING: &str = "SKILLLITE_ENABLE_TASK_PLANNING";

    // ─── Legacy (fallback when SKILLLITE_* not set) ───────────────────────────
    pub const SKILLBOX_SKILLS_ROOT: &str = "SKILLBOX_SKILLS_ROOT";
    pub const SKILLBOX_CACHE_DIR: &str = "SKILLBOX_CACHE_DIR";
    pub const AGENTSKILL_CACHE_DIR: &str = "AGENTSKILL_CACHE_DIR"; // deprecated, use SKILLLITE_CACHE_DIR
}
