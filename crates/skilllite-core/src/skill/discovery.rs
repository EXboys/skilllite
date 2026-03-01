//! Unified skill discovery: find skill directories (containing SKILL.md) in a workspace.
//!
//! Used by skill add, chat, agent-rpc, and swarm to consistently discover skills
//! across `.skills`, `skills`, `.agents/skills`, `.claude/skills`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Default directories to search for skills, relative to workspace root.
/// Includes "." to scan workspace root's direct children (e.g. for skill add from repo).
pub const SKILL_SEARCH_DIRS: &[&str] =
    &[".skills", "skills", ".agents/skills", ".claude/skills", "."];

/// Discover all skill directories in a workspace.
///
/// Searches in `search_dirs` (or `SKILL_SEARCH_DIRS` if None) for directories
/// containing SKILL.md. Deduplicates by canonical path.
///
/// Returns paths to skill directories (each has SKILL.md), sorted.
pub fn discover_skills_in_workspace(
    workspace: &Path,
    search_dirs: Option<&[&str]>,
) -> Vec<PathBuf> {
    let dirs = search_dirs.unwrap_or(SKILL_SEARCH_DIRS);
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut seen = HashSet::new();

    // If workspace itself is a skill
    if workspace.join("SKILL.md").exists() {
        if let Ok(real) = workspace.canonicalize() {
            if seen.insert(real) {
                candidates.push(workspace.to_path_buf());
            }
        }
    }

    for search_dir in dirs {
        let search_path = workspace.join(search_dir);
        if !search_path.is_dir() {
            continue;
        }
        let is_root = search_dir == &".";

        // Search path itself might be a skill (skip for "." to avoid duplicate with workspace)
        if !is_root && search_path.join("SKILL.md").exists() {
            if let Ok(real) = search_path.canonicalize() {
                if seen.insert(real) {
                    candidates.push(search_path.clone());
                }
            }
        }

        // Scan subdirectories
        let Ok(entries) = fs::read_dir(&search_path) else {
            continue;
        };
        let mut children: Vec<_> = entries.flatten().collect();
        children.sort_by_key(|e| e.file_name());
        for entry in children {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                if let Ok(real) = p.canonicalize() {
                    if seen.insert(real) {
                        candidates.push(p);
                    }
                }
            }
        }
    }

    candidates.sort();
    candidates
}

/// Discover skill directories for `load_skills`, as `Vec<String>`.
///
/// When no skills are found in subdirs, returns existing parent dirs
/// (e.g. `.skills`, `skills`) as fallback so `load_skills` can scan them.
/// This matches the previous chat/rpc behavior.
pub fn discover_skill_dirs_for_loading(
    workspace: &Path,
    search_dirs: Option<&[&str]>,
) -> Vec<String> {
    let discovered = discover_skills_in_workspace(workspace, search_dirs);
    if !discovered.is_empty() {
        return discovered
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
    }

    // Fallback: return parent dirs that exist (load_skills will scan them)
    let dirs = search_dirs.unwrap_or(SKILL_SEARCH_DIRS);
    let mut fallback = Vec::new();
    for d in dirs {
        let p = workspace.join(d);
        if p.is_dir() {
            fallback.push(p.to_string_lossy().to_string());
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_discover_skills_in_workspace_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let found = discover_skills_in_workspace(tmp.path(), Some(&[".skills", "skills"]));
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_skills_in_workspace_finds_skills() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let skill_a = skills_dir.join("skill-a");
        fs::create_dir_all(&skill_a).unwrap();
        fs::write(skill_a.join("SKILL.md"), "name: skill-a\n").unwrap();
        let skill_b = skills_dir.join("skill-b");
        fs::create_dir_all(&skill_b).unwrap();
        fs::write(skill_b.join("SKILL.md"), "name: skill-b\n").unwrap();

        let found = discover_skills_in_workspace(tmp.path(), Some(&[".skills", "skills"]));
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|p| p.ends_with("skill-a")));
        assert!(found.iter().any(|p| p.ends_with("skill-b")));
    }

    #[test]
    fn test_discover_skill_dirs_for_loading_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".skills");
        fs::create_dir_all(&skills_dir).unwrap();
        // No skills in subdirs
        let found = discover_skill_dirs_for_loading(tmp.path(), Some(&[".skills", "skills"]));
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with(".skills"));
    }
}
