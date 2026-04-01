//! Crate-level error type for `skilllite-sandbox`.

use thiserror::Error;

use crate::bash_validator::BashValidationError;

/// Unified error for sandbox operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Bash command validation failure.
    #[error(transparent)]
    BashValidation(#[from] BashValidationError),

    /// Nix syscall error (e.g. mount/unshare namespace operations).
    #[error(transparent)]
    Nix(#[from] nix::errno::Errno),

    /// Input validation / configuration error.
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

macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::Error::validation(format!($($arg)*)))
    };
}
pub(crate) use bail;
