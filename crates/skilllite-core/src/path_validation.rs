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
    let bytes = segment.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Resolve a skill-relative `entry_point` under `skill_dir`.
///
/// Nested relatives like `scripts/main.py` are allowed. Absolute paths,
/// `..`, backslashes, and Windows drive prefixes are rejected on all hosts
/// so `Path::join` cannot replace or escape the skill root.
pub fn script_path_under_skill_dir(
    skill_dir: &Path,
    entry_point: &str,
) -> Result<PathBuf, PathValidationError> {
    let escape = || PathValidationError::PathEscape {
        path_type: "Entry point".to_string(),
        path: entry_point.to_string(),
    };

    if entry_point.trim().is_empty() || entry_point.contains('\0') || entry_point.contains('\\') {
        return Err(escape());
    }
    if entry_point.split('/').any(has_windows_drive_prefix) {
        return Err(escape());
    }

    let ep = Path::new(entry_point);
    if ep.is_absolute() {
        return Err(escape());
    }

    let mut saw_normal = false;
    for component in ep.components() {
        match component {
            Component::Normal(_) => saw_normal = true,
            Component::CurDir => {}
            _ => return Err(escape()),
        }
    }
    if !saw_normal {
        return Err(escape());
    }

    let path = skill_dir.join(ep);
    if !path.starts_with(skill_dir) {
        return Err(escape());
    }
    Ok(path)
}

/// Ensure `entry_point` resolves to an existing file inside `skill_dir`.
///
/// Performs lexical containment first, then canonicalize + prefix check so
/// symlinks that leave the skill tree are also rejected.
pub fn ensure_entry_point_within_skill(
    skill_dir: &Path,
    entry_point: &str,
) -> Result<PathBuf, PathValidationError> {
    let joined = script_path_under_skill_dir(skill_dir, entry_point)?;
    let root = skill_dir
        .canonicalize()
        .map_err(|_| PathValidationError::NotFound {
            path_type: "Skill path".to_string(),
            path: skill_dir.display().to_string(),
        })?;
    let canonical = joined
        .canonicalize()
        .map_err(|_| PathValidationError::NotFound {
            path_type: "Entry point".to_string(),
            path: entry_point.to_string(),
        })?;
    if !canonical.starts_with(&root) {
        return Err(PathValidationError::PathEscape {
            path_type: "Entry point".to_string(),
            path: entry_point.to_string(),
        });
    }
    if !canonical.is_file() {
        return Err(PathValidationError::NotFound {
            path_type: "Entry point".to_string(),
            path: entry_point.to_string(),
        });
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn script_path_under_skill_dir_allows_nested_relative() {
        let skill_dir = PathBuf::from("/ws/skills/demo");
        let joined = script_path_under_skill_dir(&skill_dir, "scripts/main.py").unwrap();
        assert_eq!(joined, skill_dir.join("scripts/main.py"));
        let dotted = script_path_under_skill_dir(&skill_dir, "./main.py").unwrap();
        assert_eq!(dotted, skill_dir.join("./main.py"));
    }

    #[test]
    fn script_path_under_skill_dir_rejects_absolute_and_traversal() {
        let skill_dir = PathBuf::from("/ws/skills/demo");
        for ep in [
            "",
            " ",
            "/",
            "/tmp/pwn.py",
            "../pwn.py",
            "../../tmp/pwn.py",
            "scripts/../../pwn.py",
            "\\Windows\\Temp\\pwn.py",
            "C:\\Windows\\Temp\\pwn.py",
            "C:/Windows/Temp/pwn.py",
            "foo\0bar.py",
        ] {
            assert!(
                script_path_under_skill_dir(&skill_dir, ep).is_err(),
                "expected reject for {ep:?}"
            );
        }
    }

    #[test]
    fn ensure_entry_point_within_skill_accepts_in_tree_file() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().canonicalize().unwrap();
        fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        fs::write(skill_dir.join("scripts/main.py"), "print(1)\n").unwrap();
        let got = ensure_entry_point_within_skill(&skill_dir, "scripts/main.py").unwrap();
        assert_eq!(got, skill_dir.join("scripts/main.py"));
    }

    #[test]
    fn ensure_entry_point_within_skill_rejects_outside_file() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("skill");
        fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        let outside = dir.path().join("outside.py");
        fs::write(&outside, "print('outside')\n").unwrap();
        let skill_dir = skill_dir.canonicalize().unwrap();

        // Absolute form
        assert!(ensure_entry_point_within_skill(&skill_dir, outside.to_str().unwrap()).is_err());

        // Relative traversal to sibling file
        assert!(ensure_entry_point_within_skill(&skill_dir, "../outside.py").is_err());
    }
}
