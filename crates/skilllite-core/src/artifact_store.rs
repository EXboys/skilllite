//! Run-scoped artifact storage contract.
//!
//! Defines the `ArtifactStore` trait and structured errors.
//! Framework provides a default local directory implementation in `skilllite-agent`;
//! users may supply their own (S3, DB, etc.) by implementing the same trait.

use std::fmt;

/// Structured error for artifact store operations.
#[derive(Debug)]
pub enum StoreError {
    /// The requested run or key does not exist.
    NotFound { run_id: String, key: String },
    /// Key failed validation (empty, contains `..`, too long, etc.).
    InvalidKey { key: String, reason: String },
    /// Backend I/O or infrastructure failure.
    Backend {
        message: String,
        retryable: bool,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotFound { run_id, key } => {
                write!(f, "artifact not found: run={}, key={}", run_id, key)
            }
            StoreError::InvalidKey { key, reason } => {
                write!(f, "invalid artifact key '{}': {}", key, reason)
            }
            StoreError::Backend {
                message, retryable, ..
            } => {
                write!(
                    f,
                    "artifact store backend error (retryable={}): {}",
                    retryable, message
                )
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Backend {
                source: Some(e), ..
            } => Some(e.as_ref()),
            _ => None,
        }
    }
}

/// Run-scoped artifact store contract.
///
/// Implementations must be `Send + Sync` so the store can be shared across
/// async tasks via `Arc<dyn ArtifactStore>`.
///
/// v0 surface: `get` and `put` only. `put` uses upsert (always-overwrite)
/// semantics. Streaming, listing, TTL, and CAS are deferred.
pub trait ArtifactStore: Send + Sync {
    /// Retrieve an artifact by run scope and logical key.
    ///
    /// Returns `Ok(None)` if the key has never been written (as opposed to
    /// `StoreError::NotFound` which indicates the run scope itself is invalid
    /// or the backend cannot confirm existence).
    fn get(&self, run_id: &str, key: &str) -> Result<Option<Vec<u8>>, StoreError>;

    /// Store (or overwrite) an artifact under the given run scope and key.
    fn put(&self, run_id: &str, key: &str, data: &[u8]) -> Result<(), StoreError>;
}

/// Maximum allowed key length (bytes). Keeps filesystem paths manageable.
pub const MAX_KEY_LENGTH: usize = 512;

/// Validate an artifact key. Returns `Ok(())` or an `InvalidKey` error.
///
/// Rules:
/// - Must not be empty.
/// - Must not exceed `MAX_KEY_LENGTH`.
/// - Must not contain `..` (path traversal).
/// - Must not start with `/` or `\` (absolute path).
/// - Must not contain null bytes.
pub fn validate_artifact_key(key: &str) -> Result<(), StoreError> {
    if key.is_empty() {
        return Err(StoreError::InvalidKey {
            key: key.to_string(),
            reason: "key must not be empty".to_string(),
        });
    }
    if key.len() > MAX_KEY_LENGTH {
        return Err(StoreError::InvalidKey {
            key: key.chars().take(64).collect::<String>(),
            reason: format!("key exceeds maximum length of {} bytes", MAX_KEY_LENGTH),
        });
    }
    if key.contains("..") {
        return Err(StoreError::InvalidKey {
            key: key.to_string(),
            reason: "key must not contain '..' (path traversal)".to_string(),
        });
    }
    if key.starts_with('/') || key.starts_with('\\') {
        return Err(StoreError::InvalidKey {
            key: key.to_string(),
            reason: "key must not start with '/' or '\\'".to_string(),
        });
    }
    if key.contains('\0') {
        return Err(StoreError::InvalidKey {
            key: key.to_string(),
            reason: "key must not contain null bytes".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_keys_pass() {
        assert!(validate_artifact_key("output.json").is_ok());
        assert!(validate_artifact_key("step1/result.csv").is_ok());
        assert!(validate_artifact_key("a").is_ok());
        assert!(validate_artifact_key("deep/nested/path/file.bin").is_ok());
    }

    #[test]
    fn empty_key_rejected() {
        let err = validate_artifact_key("").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn traversal_key_rejected() {
        let err = validate_artifact_key("../etc/passwd").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
        let err = validate_artifact_key("foo/../bar").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn absolute_key_rejected() {
        let err = validate_artifact_key("/etc/passwd").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
        let err = validate_artifact_key("\\windows\\system32").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn null_byte_rejected() {
        let err = validate_artifact_key("foo\0bar").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn overlong_key_rejected() {
        let long_key = "x".repeat(MAX_KEY_LENGTH + 1);
        let err = validate_artifact_key(&long_key).unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn max_length_key_passes() {
        let key = "x".repeat(MAX_KEY_LENGTH);
        assert!(validate_artifact_key(&key).is_ok());
    }

    #[test]
    fn store_error_display() {
        let e = StoreError::NotFound {
            run_id: "abc".to_string(),
            key: "foo".to_string(),
        };
        assert!(e.to_string().contains("abc"));
        assert!(e.to_string().contains("foo"));

        let e = StoreError::Backend {
            message: "timeout".to_string(),
            retryable: true,
            source: None,
        };
        assert!(e.to_string().contains("retryable=true"));
    }

    #[test]
    fn unicode_key_passes() {
        assert!(validate_artifact_key("报告/结果.json").is_ok());
        assert!(validate_artifact_key("📊data.csv").is_ok());
    }
}
