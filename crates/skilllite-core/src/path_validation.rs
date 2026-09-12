//! Path validation utilities.
//!
//! Ensures paths stay within allowed root to prevent path traversal attacks.

use crate::error::PathValidationError;
use std::path::{Component, Path, PathBuf};

/// Get the allowed root directory for path validation.
pub fn get_allowed_root() -> Result<PathBuf, PathValidationError> {
    let allowed_root = crate::config::PathsConfig::from_env()
        .skills_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    allowed_root
        .canonicalize()
        .map_err(PathValidationError::InvalidRoot)
}

/// Validate path is within allowed root. Prevents path traversal.
pub fn validate_path_under_root(
    path: &str,
    path_type: &str,
) -> Result<PathBuf, PathValidationError> {
    let allowed_root = get_allowed_root()?;
    let input = Path::new(path);
    let full = if input.is_absolute() {
        input.to_path_buf()
    } else {
        allowed_root.join(input)
    };
    let canonical = full
        .canonicalize()
        .map_err(|_| PathValidationError::NotFound {
            path_type: path_type.to_string(),
            path: path.to_string(),
        })?;
    if !canonical.starts_with(&allowed_root) {
        return Err(PathValidationError::PathEscape {
            path_type: path_type.to_string(),
            path: path.to_string(),
        });
    }
    Ok(canonical)
}

/// Validate skill_dir is within allowed root. Prevents path traversal.
pub fn validate_skill_path(skill_dir: &str) -> Result<PathBuf, PathValidationError> {
    validate_path_under_root(skill_dir, "Skill path")
}

fn has_windows_drive_prefix(segment: &str) -> bool {
    let mut chars = segment.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(drive), Some(':')) if drive.is_ascii_alphabetic()
    )
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Validate a memory note path before joining it under `<chat_root>/memory/`.
///
/// Nested `/` relatives are allowed (`notes/day.md`). Absolute forms, `..`,
/// backslash separators, null bytes, and Windows drive prefixes are rejected on
/// every host so `Path::join` cannot replace the memory root on Windows.
pub fn validate_memory_rel_path(rel_path: &str) -> Result<&str, PathValidationError> {
    let invalid = || PathValidationError::InvalidMemoryRelPath {
        path: rel_path.to_string(),
    };

    if rel_path.is_empty() || rel_path.contains('\0') {
        return Err(invalid());
    }
    if rel_path.contains("..") {
        return Err(invalid());
    }
    if rel_path.starts_with('/') || rel_path.starts_with('\\') {
        return Err(invalid());
    }
    if rel_path.contains('\\') {
        return Err(invalid());
    }
    if rel_path.split('/').any(has_windows_drive_prefix) {
        return Err(invalid());
    }

    let path = Path::new(rel_path);
    if path.is_absolute() {
        return Err(invalid());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(invalid());
            }
        }
    }
    Ok(rel_path)
}

/// Join `rel_path` under `memory_dir` after validation and lexical containment.
pub fn memory_file_under_dir(
    memory_dir: &Path,
    rel_path: &str,
) -> Result<PathBuf, PathValidationError> {
    let rel = validate_memory_rel_path(rel_path)?;
    let full = memory_dir.join(rel);
    let normalized = normalize_lexical(&full);
    let memory_normalized = normalize_lexical(memory_dir);
    if !normalized.starts_with(&memory_normalized) {
        return Err(PathValidationError::PathEscape {
            path_type: "memory rel_path".to_string(),
            path: rel_path.to_string(),
        });
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_memory_rel_path_accepts_nested_relative() {
        assert_eq!(validate_memory_rel_path("MEMORY.md").unwrap(), "MEMORY.md");
        assert_eq!(
            validate_memory_rel_path("notes/day.md").unwrap(),
            "notes/day.md"
        );
        assert_eq!(
            validate_memory_rel_path("deep/nested/path/file.md").unwrap(),
            "deep/nested/path/file.md"
        );
    }

    #[test]
    fn validate_memory_rel_path_rejects_escape_forms() {
        for path in [
            "",
            "/tmp/pwn.md",
            "../secret.md",
            "foo/../bar.md",
            r"\Windows\Temp\pwn.md",
            r"C:\Temp\pwn.md",
            "C:/Temp/pwn.md",
            r"notes\day.md",
            "foo\0bar.md",
        ] {
            let err = validate_memory_rel_path(path).unwrap_err();
            assert!(
                matches!(err, PathValidationError::InvalidMemoryRelPath { .. }),
                "expected invalid memory rel_path for {path:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn memory_file_under_dir_stays_inside_memory_root() {
        let root = PathBuf::from("/tmp/chat/memory");
        let joined = memory_file_under_dir(&root, "notes/day.md").unwrap();
        assert_eq!(joined, root.join("notes/day.md"));
        assert!(memory_file_under_dir(&root, "C:/Temp/pwn.md").is_err());
        assert!(memory_file_under_dir(&root, "/tmp/pwn.md").is_err());
        assert!(memory_file_under_dir(&root, "../escape.md").is_err());
    }
}
