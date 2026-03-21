//! Unified error type for the `skilllite` CLI crate (`thiserror`).
//!
//! Workspace crates that still return `anyhow::Error` are wrapped in [`Error::Other`].

use thiserror::Error;

/// Result type used throughout the `skilllite` library.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error for [`crate::run_cli`] and internal protocol/command paths.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O failures (filesystem, stdin/stdout, etc.).
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON parse/serialize errors.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// Skill path validation (workspace root rules).
    #[error(transparent)]
    PathValidation(#[from] skilllite_core::error::PathValidationError),

    /// Additional context wrapping an inner [`Error`].
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<Error>,
    },

    /// Validation or protocol misuse (replaces ad-hoc `anyhow::bail!` in this crate).
    #[error("{0}")]
    Message(String),

    /// Errors from workspace crates that use `anyhow` (`?` converts automatically).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    pub fn msg(s: impl Into<String>) -> Self {
        Error::Message(s.into())
    }

    pub fn with_context<C: std::fmt::Display, E: Into<Error>>(context: C, err: E) -> Self {
        Error::Context {
            context: context.to_string(),
            source: Box::new(err.into()),
        }
    }
}
