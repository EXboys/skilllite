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

/// Validate a memory agent ID before joining it under `memory/{agent_id}.sqlite`.
///
/// Accepts only a single normal path component. Rejects empty IDs, `.` / `..`,
/// separators, null bytes, and absolute / multi-segment forms that could escape
/// the chat memory root via `Path::join` (including Windows drive prefixes).
pub fn validate_agent_id(agent_id: &str) -> Result<&str, PathValidationError> {
    let invalid = || PathValidationError::InvalidAgentId {
        id: agent_id.to_string(),
    };

    if agent_id.trim().is_empty() || agent_id.chars().any(|c| matches!(c, '/' | '\\' | '\0')) {
        return Err(invalid());
    }

    // Reject Windows drive-like prefixes even on non-Windows hosts.
    let bytes = agent_id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(invalid());
    }

    let mut components = Path::new(agent_id).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(agent_id),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_agent_id_accepts_single_segment() {
        assert_eq!(validate_agent_id("default").unwrap(), "default");
        assert_eq!(validate_agent_id("agent-1").unwrap(), "agent-1");
        assert_eq!(validate_agent_id("会话-1").unwrap(), "会话-1");
    }

    #[test]
    fn validate_agent_id_rejects_traversal_and_separators() {
        for id in [
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
            let err = validate_agent_id(id).unwrap_err();
            assert!(
                matches!(err, PathValidationError::InvalidAgentId { .. }),
                "expected invalid agent_id for {id:?}, got {err:?}"
            );
        }
    }
}
