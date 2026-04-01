//! Crate-level error type for `skilllite-fs`.

use thiserror::Error;

/// Unified error for file-system operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Input validation failure (empty search string, path not a directory, etc.).
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
