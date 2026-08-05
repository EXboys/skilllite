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
        Ok(self.base_dir.join("artifacts").join(run_id).join(key))
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

/// Staging path for an atomic write.
///
/// Must stay unique per destination basename: `Path::with_extension("tmp")`
/// collapses `report.json` and `report.csv` onto the same `report.tmp`, and
/// leaves a key ending in `.tmp` with no distinct staging file.
fn staging_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "artifact".to_string());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        nanos
    ))
}

/// Atomic write for byte data: write to temp file, then rename.
fn atomic_write_bytes(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    let tmp = staging_path_for(path);
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

    #[test]
    fn staging_paths_preserve_full_basename() {
        let json = PathBuf::from("/tmp/artifacts/run/report.json");
        let csv = PathBuf::from("/tmp/artifacts/run/report.csv");
        let tmp_key = PathBuf::from("/tmp/artifacts/run/report.tmp");

        let json_tmp = staging_path_for(&json);
        let csv_tmp = staging_path_for(&csv);
        let tmp_key_tmp = staging_path_for(&tmp_key);

        let json_name = json_tmp.file_name().unwrap().to_string_lossy();
        let csv_name = csv_tmp.file_name().unwrap().to_string_lossy();
        let tmp_key_name = tmp_key_tmp.file_name().unwrap().to_string_lossy();

        assert!(
            json_name.contains("report.json"),
            "staging name should retain full basename, got {json_name}"
        );
        assert!(
            csv_name.contains("report.csv"),
            "staging name should retain full basename, got {csv_name}"
        );
        assert_ne!(
            json_tmp.file_name(),
            csv_tmp.file_name(),
            "keys that share a stem must not collide on the staging path"
        );
        assert_ne!(
            tmp_key_tmp, tmp_key,
            "keys ending in .tmp must still get a distinct staging path"
        );
        assert!(
            tmp_key_name.contains("report.tmp"),
            "staging name should retain .tmp basename, got {tmp_key_name}"
        );
    }

    #[test]
    fn put_keeps_sibling_stem_keys_independent() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "report.json", b"json-bytes").unwrap();
        store.put("run-1", "report.csv", b"csv-bytes").unwrap();
        assert_eq!(
            store.get("run-1", "report.json").unwrap(),
            Some(b"json-bytes".to_vec())
        );
        assert_eq!(
            store.get("run-1", "report.csv").unwrap(),
            Some(b"csv-bytes".to_vec())
        );
    }

    #[test]
    fn put_tmp_extension_key_roundtrips_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        store.put("run-1", "report.tmp", b"staged-ok").unwrap();
        assert_eq!(
            store.get("run-1", "report.tmp").unwrap(),
            Some(b"staged-ok".to_vec())
        );
    }

    #[test]
    fn concurrent_puts_for_shared_stem_keys_preserve_payloads() {
        use std::sync::Arc;
        use std::thread;

        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(make_store(&dir));
        let json_store = Arc::clone(&store);
        let csv_store = Arc::clone(&store);

        let json_thread = thread::spawn(move || {
            for i in 0..40 {
                let payload = format!("json-{i}");
                json_store
                    .put("run-1", "report.json", payload.as_bytes())
                    .unwrap();
            }
        });
        let csv_thread = thread::spawn(move || {
            for i in 0..40 {
                let payload = format!("csv-{i}");
                csv_store
                    .put("run-1", "report.csv", payload.as_bytes())
                    .unwrap();
            }
        });
        json_thread.join().unwrap();
        csv_thread.join().unwrap();

        let json = store.get("run-1", "report.json").unwrap().unwrap();
        let csv = store.get("run-1", "report.csv").unwrap().unwrap();
        let json_text = String::from_utf8(json).unwrap();
        let csv_text = String::from_utf8(csv).unwrap();
        assert!(
            json_text.starts_with("json-"),
            "report.json must not receive csv payload, got {json_text}"
        );
        assert!(
            csv_text.starts_with("csv-"),
            "report.csv must not receive json payload, got {csv_text}"
        );
    }
}
