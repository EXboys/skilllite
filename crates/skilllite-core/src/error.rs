//! Crate-level error types for `skilllite-core`.
//!
//! [`PathValidationError`] is kept as a standalone type for backward compatibility
//! (external crates may match on its variants directly).
//! [`Error`] is the unified crate-level error used by all other public APIs.

use thiserror::Error;

// ── Crate-level error ────────────────────────────────────────────────────────

/// Unified error for `skilllite-core` public APIs.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Path validation failure.
    #[error(transparent)]
    PathValidation(#[from] PathValidationError),

    /// Error from `skilllite-fs` operations.
    #[error(transparent)]
    Fs(#[from] skilllite_fs::Error),

    /// JSON serialization / deserialization.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// YAML serialization / deserialization.
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    /// Input validation / business-rule violation.
    #[error("{0}")]
    Validation(String),

    /// Catch-all for internal `anyhow` usage during gradual migration.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Crate-level `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn validation(msg: impl Into<String>) -> Self {
        Error::Validation(msg.into())
    }
}

// ── Path validation errors (standalone, backward-compatible) ─────────────────

/// Errors from path validation operations.
///
/// Used by `get_allowed_root`, `validate_path_under_root`, `validate_skill_path`.
/// Kept as a separate type so callers can match on variants directly.
#[derive(Debug, Error)]
pub enum PathValidationError {
    /// The configured SKILLLITE_SKILLS_ROOT (or cwd) could not be resolved.
    #[error("Invalid SKILLLITE_SKILLS_ROOT: {0}")]
    InvalidRoot(#[from] std::io::Error),

    /// Path does not exist.
    #[error("{path_type} does not exist: {path}")]
    NotFound { path_type: String, path: String },

    /// Path escapes the allowed root (potential path traversal).
    #[error("{path_type} escapes allowed root: {path}")]
    PathEscape { path_type: String, path: String },
}
