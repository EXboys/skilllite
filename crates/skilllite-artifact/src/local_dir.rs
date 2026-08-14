//! Local filesystem [`ArtifactStore`](skilllite_core::artifact_store::ArtifactStore).

use skilllite_core::artifact_store::{validate_artifact_key, ArtifactStore, StoreError};
use std::path::{Path, PathBuf};

use crate::validation::validate_run_id;

/// Local filesystem artifact store.
///
/// Layout: `<base_dir>/artifacts/<run_id>/<key>`
///
/// Keys may contain `/` for logical grouping (e.g. `step1/output.json`);
/// each segment is validated against path-traversal rules.
pub struct LocalDirArtifactStore {
    base_dir: PathBuf,
}

impl LocalDirArtifactStore {
    /// Create a store rooted at `base_dir`.
    /// Artifacts are written under `<base_dir>/artifacts/<run_id>/<key>`.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn artifact_path(&self, run_id: &str, key: &str) -> Result<PathBuf, StoreError> {
        validate_artifact_key(key)?;
        validate_run_id(run_id)?;

        // Platform-specific absolute forms (especially Windows) must never replace the root.
        if Path::new(run_id).is_absolute() || Path::new(key).is_absolute() {
            return Err(StoreError::InvalidKey {
                key: format!("{run_id}/{key}"),
                reason: "artifact path components must be relative".to_string(),
            });
        }

        let root = self.base_dir.join("artifacts");
        let path = root.join(run_id).join(key);
        if !path.starts_with(&root) {
            return Err(StoreError::InvalidKey {
                key: format!("{run_id}/{key}"),
                reason: "resolved artifact path escapes store root".to_string(),
            });
        }
        Ok(path)
    }
}

impl ArtifactStore for LocalDirArtifactStore {
    fn get(&self, run_id: &str, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.artifact_path(run_id, key)?;
        match std::fs::read(&path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Backend {
                message: format!("failed to read {}: {}", path.display(), e),
                retryable: false,
                source: Some(Box::new(e)),
            }),
        }
    }

    fn put(&self, run_id: &str, key: &str, data: &[u8]) -> Result<(), StoreError> {
        let path = self.artifact_path(run_id, key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::Backend {
                message: format!("failed to create directory {}: {}", parent.display(), e),
                retryable: false,
                source: Some(Box::new(e)),
            })?;
        }
        atomic_write_bytes(&path, data)
    }
}

/// Atomic write for byte data: write to temp file, then rename.
fn atomic_write_bytes(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| StoreError::Backend {
        message: format!("failed to write temp file {}: {}", tmp.display(), e),
        retryable: false,
        source: Some(Box::new(e)),
    })?;
    std::fs::rename(&tmp, path).map_err(|e| StoreError::Backend {
        message: format!(
            "failed to rename {} -> {}: {}",
            tmp.display(),
            path.display(),
            e
        ),
        retryable: false,
        source: Some(Box::new(e)),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store(dir: &tempfile::TempDir) -> LocalDirArtifactStore {
        LocalDirArtifactStore::new(dir.path())
    }

    #[test]
    fn put_and_get_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "output.json", b"hello").unwrap();
        let data = store.get("run-1", "output.json").unwrap();
        assert_eq!(data, Some(b"hello".to_vec()));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let data = store.get("run-1", "nonexistent").unwrap();
        assert_eq!(data, None);
    }

    #[test]
    fn put_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "data.bin", b"v1").unwrap();
        store.put("run-1", "data.bin", b"v2").unwrap();
        let data = store.get("run-1", "data.bin").unwrap();
        assert_eq!(data, Some(b"v2".to_vec()));
    }

    #[test]
    fn hierarchical_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "step1/output.json", b"nested").unwrap();
        let data = store.get("run-1", "step1/output.json").unwrap();
        assert_eq!(data, Some(b"nested".to_vec()));
    }

    #[test]
    fn invalid_key_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let err = store.put("run-1", "../escape", b"bad").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn empty_key_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let err = store.put("run-1", "", b"bad").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn invalid_run_id_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let err = store.put("../bad", "key", b"data").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
        let err = store.put("", "key", b"data").unwrap_err();
        assert!(matches!(err, StoreError::InvalidKey { .. }));
    }

    #[test]
    fn windows_escape_keys_and_run_ids_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let marker = dir.path().join("outside-marker.txt");
        std::fs::write(&marker, b"keep").unwrap();

        for (run_id, key) in [
            ("run-a", r"C:\Users\Public\owned.txt"),
            ("run-a", "C:/Users/Public/owned.txt"),
            (r"\Windows\Temp", "x.txt"),
            (r"C:\evil", "x.txt"),
        ] {
            let err = store.put(run_id, key, b"pwn").unwrap_err();
            assert!(
                matches!(err, StoreError::InvalidKey { .. }),
                "expected rejection for ({run_id:?}, {key:?}), got {err}"
            );
        }

        // Rejected puts must not mutate unrelated files outside the store root.
        assert_eq!(std::fs::read(&marker).unwrap(), b"keep");
    }

    #[test]
    fn separate_runs_isolated() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-a", "data", b"alpha").unwrap();
        store.put("run-b", "data", b"beta").unwrap();
        assert_eq!(store.get("run-a", "data").unwrap(), Some(b"alpha".to_vec()));
        assert_eq!(store.get("run-b", "data").unwrap(), Some(b"beta".to_vec()));
    }

    #[test]
    fn binary_data_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let binary: Vec<u8> = (0..=255).collect();
        store.put("run-1", "binary.bin", &binary).unwrap();
        let data = store.get("run-1", "binary.bin").unwrap();
        assert_eq!(data, Some(binary));
    }

    #[test]
    fn unicode_key_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "报告/结果.json", b"data").unwrap();
        let data = store.get("run-1", "报告/结果.json").unwrap();
        assert_eq!(data, Some(b"data".to_vec()));
    }
}
