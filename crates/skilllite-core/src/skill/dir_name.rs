//! Skill directory names that are safe to join onto a skills root.
//!
//! Install and remove join this name and may delete the destination. A name
//! that is absolute, `.`, `..`, or contains a separator would address a
//! directory outside the skills root.

use std::path::{Component, Path};

use crate::{Error, Result};

/// Accept a single directory name such as `my-skill` or `笔记`.
///
/// Reject path separators, `.`, `..`, absolute paths, and control characters.
pub fn validate_skill_dir_name(name: &str) -> Result<()> {
    if !is_single_directory_name(name) {
        return Err(Error::validation(format!(
            "skill name '{name}' must be a single directory name"
        )));
    }
    Ok(())
}

/// A relative path that stays inside its parent after `join`.
///
/// `scripts/main.py` is accepted. Absolute paths and `..` are rejected.
pub fn is_relative_path_inside(path: &str) -> bool {
    if path.is_empty() || path.chars().any(|c| c.is_control()) {
        return false;
    }
    let path = Path::new(path);
    if path.is_absolute() {
        return false;
    }
    let mut saw_normal = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => saw_normal = true,
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return false;
            }
        }
    }
    saw_normal
}

fn is_single_directory_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 || name.chars().any(|c| c.is_control()) {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }
    match (path.components().next(), path.components().nth(1)) {
        (Some(Component::Normal(part)), None) => {
            let text = part.to_string_lossy();
            !text.is_empty() && text != "." && text != ".."
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_hyphenated_and_cjk_names() {
        assert!(validate_skill_dir_name("my-skill").is_ok());
        assert!(validate_skill_dir_name("nano-banana-pro").is_ok());
        assert!(validate_skill_dir_name("笔记").is_ok());
    }

    #[test]
    fn rejects_traversal_and_absolute_names() {
        for name in [
            "",
            ".",
            "..",
            "../.config",
            "../../.config",
            "/tmp/x",
            "foo/bar",
            "foo\\bar",
        ] {
            let err = validate_skill_dir_name(name).unwrap_err();
            assert!(
                err.to_string().contains("single directory name"),
                "{name}: {err}"
            );
        }
    }

    #[test]
    fn relative_entry_point_may_use_one_subdirectory() {
        assert!(is_relative_path_inside("scripts/main.py"));
        assert!(!is_relative_path_inside("../main.py"));
        assert!(!is_relative_path_inside("/tmp/main.py"));
        assert!(!is_relative_path_inside(""));
    }
}
