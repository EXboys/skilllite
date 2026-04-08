//! Crate-level error type for `skilllite-artifact`.

use skilllite_core::artifact_store::StoreError;
use thiserror::Error;

/// Unified error for artifact store implementations and the HTTP serve helper.
#[derive(Debug, Error)]
pub enum Error {
    /// Filesystem or TCP bind/serve I/O.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Invalid blocking HTTP client base URL or `reqwest` client builder failure.
    #[error("invalid HTTP client configuration: {0}")]
    InvalidClientConfig(String),

    /// Run ID / key validation (aligned with [`ArtifactStore`](skilllite_core::artifact_store::ArtifactStore) contract).
    #[error(transparent)]
    Store(#[from] StoreError),

    /// Validation or internal misuse.
    #[error("{0}")]
    Validation(String),

    /// Catch-all for gradual migration and context chaining.
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
