//! Structured error types for public API boundaries.
//!
//! Use thiserror for APIs consumed by other crates, enabling:
//! - Precise error matching (e.g. retry on NotFound, abort on PathEscape)
//! - Clear error messages without losing context
//! - Automatic conversion to anyhow via `?` in caller code

use thiserror::Error;

// ── Path validation errors ───────────────────────────────────────────────────

/// Errors from path validation operations.
///
/// Used by `get_allowed_root`, `validate_path_under_root`, `validate_skill_path`.
#[derive(Debug, Error)]
pub enum PathValidationError {
    /// The configured SKILLLITE_SKILLS_ROOT (or cwd) could not be resolved.
    #[error("Invalid SKILLLITE_SKILLS_ROOT: {0}")]
    InvalidRoot(#[from] std::io::Error),

    /// Path does not exist.
    #[error("{path_type} does not exist: {path}")]
    NotFound {
        /// Human-readable type (e.g. "Skill path", "Script path")
        path_type: String,
        /// The invalid path
        path: String,
    },

    /// Path escapes the allowed root (potential path traversal).
    #[error("{path_type} escapes allowed root: {path}")]
    PathEscape {
        /// Human-readable type
        path_type: String,
        /// The escaping path
        path: String,
    },
}
