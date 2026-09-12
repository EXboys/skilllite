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

/// Validate a skill directory name before joining it under a skills root.
///
/// Accepts only a single normal path component. Rejects empty names, `.` / `..`,
/// separators, null bytes, and absolute / multi-segment forms that could escape
/// the skills root via `Path::join`.
pub fn validate_skill_dir_name(name: &str) -> Result<&str, PathValidationError> {
    let invalid = || PathValidationError::InvalidSkillDirName {
        name: name.to_string(),
    };

    if name.trim().is_empty() || name.chars().any(|c| matches!(c, '/' | '\\' | '\0')) {
        return Err(invalid());
    }

    // Reject Windows drive-like prefixes even on non-Windows hosts.
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(invalid());
    }

    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(name),
        _ => Err(invalid()),
    }
}

/// Join `skill_name` under `skills_root` after validating it is a single segment.
pub fn skill_dir_under_root(
    skills_root: &Path,
    skill_name: &str,
) -> Result<PathBuf, PathValidationError> {
    let name = validate_skill_dir_name(skill_name)?;
    Ok(skills_root.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_skill_dir_name_accepts_single_segment() {
        assert_eq!(validate_skill_dir_name("calculator").unwrap(), "calculator");
        assert_eq!(validate_skill_dir_name("报告技能").unwrap(), "报告技能");
        assert_eq!(validate_skill_dir_name("my-skill_1").unwrap(), "my-skill_1");
    }

    #[test]
    fn validate_skill_dir_name_rejects_traversal_and_separators() {
        for name in [
            "",
            " ",
            ".",
            "..",
            "../evil",
            "..\\evil",
            "foo/bar",
            "foo\\bar",
            "/abs",
            "\\abs",
            "C:\\windows",
            "C:/windows",
            "foo\0bar",
        ] {
            let err = validate_skill_dir_name(name).unwrap_err();
            assert!(
                matches!(err, PathValidationError::InvalidSkillDirName { .. }),
                "expected invalid name for {name:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn skill_dir_under_root_stays_inside_skills_root() {
        let root = PathBuf::from("/tmp/skills");
        let joined = skill_dir_under_root(&root, "calculator").unwrap();
        assert_eq!(joined, root.join("calculator"));
        assert!(skill_dir_under_root(&root, "../evil").is_err());
    }
}
