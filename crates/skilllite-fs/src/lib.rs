//! SkillLite FS: 中心化文件操作
//!
//! 模块：
//! - read_write: read_file, write_file, append_file, atomic_write
//! - dir: read_dir, list_directory, file_exists, create_dir_all, copy, rename, remove_file, modified_time
//! - grep: grep_directory
//! - search_replace: apply_search_replace, apply_replace_fuzzy, insert_lines_at
//! - backup: backup_file, prune_oldest_files
//! - util: is_likely_binary, matches_glob

mod read_write;
mod dir;
mod util;
mod grep;
mod search_replace;
mod backup;

// Re-export public API
pub use read_write::{read_file, read_bytes, read_bytes_limit, write_file, append_file, atomic_write, search_replace as search_replace_file};
pub use dir::{
    read_dir, create_dir_all, copy, rename, remove_file, modified_time, file_exists,
    list_directory, PathKind,
};
pub use util::{is_likely_binary, matches_glob};
pub use grep::{grep_directory, GrepMatch, SKIP_DIRS};
pub use search_replace::{
    apply_search_replace, apply_replace_fuzzy, apply_replace_normalize_whitespace,
    insert_lines_at, line_byte_offsets, safe_excerpt, build_failure_hint, FuzzyReplaceResult,
};
pub use backup::{backup_file, prune_oldest_files};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_file_exists() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("a.txt");
        std::fs::write(&f, "x").unwrap();
        assert!(matches!(file_exists(&f).unwrap(), PathKind::File(1)));
        assert!(matches!(file_exists(dir.path()).unwrap(), PathKind::Dir));
        assert!(matches!(file_exists(&dir.path().join("nonexistent")).unwrap(), PathKind::NotFound));
    }

    #[test]
    fn test_list_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "1").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub").join("b.txt"), "2").unwrap();
        let entries = list_directory(dir.path(), false).unwrap();
        assert!(entries.iter().any(|e| e.contains("a.txt")));
        assert!(entries.iter().any(|e| e.contains("sub")));
        let rec = list_directory(dir.path(), true).unwrap();
        assert!(rec.iter().any(|e| e.contains("b.txt")));
    }

    #[test]
    fn test_apply_search_replace_once() {
        let content = "hello world\nhello rust";
        let (new_content, count) = apply_search_replace(content, "hello", "hi", false).unwrap();
        assert_eq!(new_content, "hi world\nhello rust");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_apply_search_replace_all() {
        let content = "hello world hello";
        let (new_content, count) = apply_search_replace(content, "hello", "hi", true).unwrap();
        assert_eq!(new_content, "hi world hi");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_apply_search_replace_not_found() {
        let content = "hello world";
        let r = apply_search_replace(content, "xyz", "a", false);
        assert!(r.is_err());
    }

    #[test]
    fn test_search_replace_file() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "foo bar foo").unwrap();
        let path = f.path();
        let count = search_replace_file(path, "foo", "baz", true).unwrap();
        assert_eq!(count, 2);
        let content = read_file(path).unwrap();
        assert_eq!(content, "baz bar baz");
    }
}
