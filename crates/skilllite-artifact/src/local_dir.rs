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

/// Atomic write for byte data: write to a temp file in the same directory, then rename.
///
/// The temp name is not `path.with_extension("tmp")`. That path is a legal
/// artifact key (`notes.json` would replace `notes.tmp`, and `archive.tar.gz`
/// would replace `archive.tar.tmp`).
fn atomic_write_bytes(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    let tmp = partial_path(path);
    if let Err(err) = std::fs::write(&tmp, data) {
        let _ = std::fs::remove_file(&tmp);
        return Err(StoreError::Backend {
            message: format!("failed to write temp file {}: {}", tmp.display(), err),
            retryable: false,
            source: Some(Box::new(err)),
        });
    }
    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(StoreError::Backend {
            message: format!(
                "failed to rename {} -> {}: {}",
                tmp.display(),
                path.display(),
                err
            ),
            retryable: false,
            source: Some(Box::new(err)),
        });
    }
    Ok(())
}

fn partial_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let name = format!(".skilllite-partial-{}-{nanos}-{seq}", std::process::id());
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(name)
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
    fn put_keeps_sibling_keys_that_share_the_old_temp_stem() {
        let dir = tempfile::tempdir().unwrap();
        let store = make_store(&dir);
        let notes_tmp = "保留 中文".as_bytes();
        store.put("run-1", "notes.tmp", notes_tmp).unwrap();
        store.put("run-1", "notes.json", b"{\"ok\":true}").unwrap();
        store.put("run-1", "archive.tar.tmp", b"tar-tmp").unwrap();
        store.put("run-1", "archive.tar.gz", b"tar-gz").unwrap();
        store.put("run-1", "report.tmp", b"report-tmp").unwrap();
        store.put("run-1", "report", b"report-final").unwrap();

        assert_eq!(
            store.get("run-1", "notes.tmp").unwrap().as_deref(),
            Some(notes_tmp)
        );
        assert_eq!(
            store.get("run-1", "notes.json").unwrap().as_deref(),
            Some(b"{\"ok\":true}".as_slice())
        );
        assert_eq!(
            store.get("run-1", "archive.tar.tmp").unwrap().as_deref(),
            Some(b"tar-tmp".as_slice())
        );
        assert_eq!(
            store.get("run-1", "archive.tar.gz").unwrap().as_deref(),
            Some(b"tar-gz".as_slice())
        );
        assert_eq!(
            store.get("run-1", "report.tmp").unwrap().as_deref(),
            Some(b"report-tmp".as_slice())
        );
        assert_eq!(
            store.get("run-1", "report").unwrap().as_deref(),
            Some(b"report-final".as_slice())
        );

        let run_dir = dir.path().join("artifacts").join("run-1");
        let partials: Vec<_> = std::fs::read_dir(&run_dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".skilllite-partial-"))
            .collect();
        assert!(partials.is_empty(), "leftover temp files: {partials:?}");
    }

    #[test]
    fn failed_rename_removes_the_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("notes.json");
        std::fs::create_dir(&dest).unwrap();
        let err = atomic_write_bytes(&dest, "不会留下 临时文件".as_bytes()).unwrap_err();
        assert!(matches!(err, StoreError::Backend { .. }));
        let partials: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".skilllite-partial-"))
            .collect();
        assert!(partials.is_empty(), "leftover temp files: {partials:?}");
    }
}
