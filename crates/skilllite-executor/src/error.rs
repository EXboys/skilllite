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
