//! Crate-level error type for `skilllite-services`.
//!
//! Per Phase 0 D3 (TASK-2026-042): each crate defines its own `thiserror`
//! enum; entry adapters convert into `anyhow::Result` (CLI) or structured
//! Tauri errors (Desktop) at the boundary. Domain errors are wrapped with
//! `#[from]` so service-layer code can use the `?` operator naturally.

use thiserror::Error;

/// Errors returned from any `skilllite-services` API.
#[derive(Debug, Error)]
pub enum Error {
    /// Caller supplied an invalid argument that could not be auto-recovered
    /// (e.g. an empty workspace path string after trimming).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Underlying I/O failure (filesystem read, etc.).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Convenience alias for service results.
pub type Result<T> = std::result::Result<T, Error>;
