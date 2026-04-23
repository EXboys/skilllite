//! Crate-level errors for outbound channel HTTP calls.

use thiserror::Error;

/// Errors from `skilllite-channel` HTTP integrations.
#[derive(Debug, Error)]
pub enum Error {
    /// Invalid URL, empty token, or other caller input.
    #[error("{0}")]
    Validation(String),

    /// HTTP transport or non-success status from the remote API.
    #[error("HTTP error: {0}")]
    Http(String),

    /// JSON encode/decode failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),

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

    pub fn http(msg: impl Into<String>) -> Self {
        Error::Http(msg.into())
    }
}

#[cfg(feature = "http")]
macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::error::Error::validation(format!($($arg)*)))
    };
}
#[cfg(feature = "http")]
pub(crate) use bail;
