//! Structured error types for executor public APIs.
//!
//! Enables precise error handling at the executor boundary without
//! coupling callers to internal implementation details.

use std::path::PathBuf;
use thiserror::Error;

/// Errors from workspace/chat root resolution.
#[derive(Debug, Error)]
pub enum ExecutorError {
    /// I/O error while resolving paths (e.g. current_dir).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Path resolution produced an invalid result.
    #[error("Invalid path: {path}")]
    InvalidPath { path: PathBuf },
}

/// Crate-level error type for `skilllite-executor`.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Executor(#[from] ExecutorError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error("{0}")]
    Validation(String),

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

macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::error::Error::validation(format!($($arg)*)))
    };
}
pub(crate) use bail;
