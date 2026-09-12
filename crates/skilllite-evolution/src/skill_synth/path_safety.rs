//! Path containment helpers for evolution skill synthesis.
//!
//! Generated skill names must be single path segments. Entry points may be
//! nested (`scripts/main.py`) but must stay under the target skill directory.

use std::path::{Component, Path, PathBuf};

use crate::error::bail;
use crate::Result;

/// Validate a generated/pending skill directory name as a single normal segment.
pub fn validate_generated_skill_name(skill_name: &str) -> Result<&str> {
    if skill_name.trim().is_empty() || skill_name.chars().any(|c| matches!(c, '/' | '\\' | '\0')) {
        bail!("invalid generated skill name: {}", skill_name);
    }

    // Reject Windows drive-like prefixes even on non-Windows hosts.
    let bytes = skill_name.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        bail!("invalid generated skill name: {}", skill_name);
    }

    let mut components = Path::new(skill_name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(skill_name),
        _ => bail!("invalid generated skill name: {}", skill_name),
    }
}

fn has_windows_drive_prefix(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Resolve `entry_point` under `skill_dir`, rejecting absolute/traversal forms.
pub fn script_path_under_skill_dir(skill_dir: &Path, entry_point: &str) -> Result<PathBuf> {
    if entry_point.trim().is_empty() || entry_point.contains('\0') || entry_point.contains('\\') {
        bail!("invalid entry_point: {}", entry_point);
    }
    if entry_point.split('/').any(has_windows_drive_prefix) {
        bail!("invalid entry_point: {}", entry_point);
    }

    let ep = Path::new(entry_point);
    if ep.is_absolute() {
        bail!("invalid entry_point: {}", entry_point);
    }

    let mut saw_normal = false;
    for component in ep.components() {
        match component {
            Component::Normal(_) => saw_normal = true,
            Component::CurDir => {}
            _ => bail!("invalid entry_point: {}", entry_point),
        }
    }
    if !saw_normal {
        bail!("invalid entry_point: {}", entry_point);
    }

    let path = skill_dir.join(ep);
    if Path::new(entry_point).is_absolute() || !path.starts_with(skill_dir) {
        bail!("entry_point escapes skill directory: {}", entry_point);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn validate_generated_skill_name_accepts_single_segment() {
        assert_eq!(
            validate_generated_skill_name("url-summarizer").unwrap(),
            "url-summarizer"
        );
        assert_eq!(
            validate_generated_skill_name("报告技能").unwrap(),
            "报告技能"
        );
    }

    #[test]
    fn validate_generated_skill_name_rejects_traversal_and_separators() {
        for name in [
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
            assert!(
                validate_generated_skill_name(name).is_err(),
                "expected invalid skill name for {name:?}"
            );
        }
    }

    #[test]
    fn script_path_under_skill_dir_allows_nested_relative() {
        let skill_dir = PathBuf::from("/ws/skills/_evolved/_pending/demo");
        let joined = script_path_under_skill_dir(&skill_dir, "scripts/main.py").unwrap();
        assert_eq!(joined, skill_dir.join("scripts/main.py"));
        let dotted = script_path_under_skill_dir(&skill_dir, "./main.py").unwrap();
        assert_eq!(dotted, skill_dir.join("./main.py"));
    }

    #[test]
    fn script_path_under_skill_dir_rejects_absolute_and_traversal() {
        let skill_dir = PathBuf::from("/ws/skills/_evolved/_pending/demo");
        for ep in [
            "",
            "/",
            "/tmp/pwn.py",
            "../../../tmp/pwn.py",
            "..\\pwn.py",
            "scripts/../../outside.py",
            "C:/Windows/Temp/pwn.py",
            "C:\\Windows\\Temp\\pwn.py",
            ".",
            "..",
            "foo\0bar.py",
        ] {
            assert!(
                script_path_under_skill_dir(&skill_dir, ep).is_err(),
                "expected invalid entry_point for {ep:?}"
            );
        }
    }

    #[test]
    #[allow(clippy::join_absolute_paths)]
    fn absolute_entry_point_does_not_replace_skill_dir() {
        let skill_dir = PathBuf::from("/ws/skills/_evolved/_pending/demo");
        // Precondition: Path::join with absolute second component replaces the root.
        assert_eq!(skill_dir.join("/tmp/pwn.py"), PathBuf::from("/tmp/pwn.py"));
        assert!(script_path_under_skill_dir(&skill_dir, "/tmp/pwn.py").is_err());
    }
}
