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

/// Validate a chat session key before joining it under transcripts/plans directories.
///
/// Accepts only a single normal path component. Rejects empty keys, `.` / `..`,
/// separators, null bytes, and absolute / multi-segment forms that could escape
/// the chat data root via `Path::join` (including Windows drive prefixes).
pub fn validate_session_key(session_key: &str) -> Result<&str, PathValidationError> {
    let invalid = || PathValidationError::InvalidSessionKey {
        key: session_key.to_string(),
    };

    if session_key.trim().is_empty() || session_key.chars().any(|c| matches!(c, '/' | '\\' | '\0'))
    {
        return Err(invalid());
    }

    // Reject Windows drive-like prefixes even on non-Windows hosts.
    let bytes = session_key.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(invalid());
    }

    let mut components = Path::new(session_key).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(session_key),
        _ => Err(invalid()),
    }
}

/// Join a filename built from `session_key` under `dir` after validation.
pub fn session_file_under_dir(
    dir: &Path,
    session_key: &str,
    file_name: &str,
) -> Result<PathBuf, PathValidationError> {
    let key = validate_session_key(session_key)?;
    // Keep the validated key in the filename so callers can pass preformatted names.
    if !file_name.starts_with(key) {
        return Err(PathValidationError::InvalidSessionKey {
            key: session_key.to_string(),
        });
    }
    let path = dir.join(file_name);
    if Path::new(file_name).is_absolute() || !path.starts_with(dir) {
        return Err(PathValidationError::PathEscape {
            path_type: "session file".to_string(),
            path: file_name.to_string(),
        });
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_session_key_accepts_single_segment() {
        assert_eq!(validate_session_key("default").unwrap(), "default");
        assert_eq!(validate_session_key("s-1a2b").unwrap(), "s-1a2b");
        assert_eq!(validate_session_key("会话-1").unwrap(), "会话-1");
        assert_eq!(
            validate_session_key("schedule-morning").unwrap(),
            "schedule-morning"
        );
    }

    #[test]
    fn validate_session_key_rejects_traversal_and_separators() {
        for key in [
            "",
            " ",
            ".",
            "..",
            "../evil",
            "..\\evil",
            "foo/bar",
            "foo\\bar",
            "/tmp/evil",
            "\\Windows\\Temp",
            "C:\\Windows\\Temp",
            "C:/Windows/Temp",
            "foo\0bar",
        ] {
            let err = validate_session_key(key).unwrap_err();
            assert!(
                matches!(err, PathValidationError::InvalidSessionKey { .. }),
                "expected invalid session key for {key:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn session_file_under_dir_stays_inside_dir() {
        let root = PathBuf::from("/tmp/chat/transcripts");
        let joined = session_file_under_dir(&root, "default", "default-2026-07-28.jsonl").unwrap();
        assert_eq!(joined, root.join("default-2026-07-28.jsonl"));
        assert!(session_file_under_dir(&root, "/tmp/evil", "/tmp/evil-2026-07-28.jsonl").is_err());
        assert!(session_file_under_dir(&root, "../evil", "../evil-2026-07-28.jsonl").is_err());
    }
}
