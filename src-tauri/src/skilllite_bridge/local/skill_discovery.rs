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
    &["skills", ".skills", ".agents/skills", ".claude/skills", "."];

/// Unified result for resolving a skills directory with legacy fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsDirResolution {
    pub requested_path: PathBuf,
    pub effective_path: PathBuf,
    pub used_legacy_fallback: bool,
    pub conflicting_skill_names: Vec<String>,
}

/// Concrete skill instance discovered in a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstance {
    pub name: String,
    pub path: PathBuf,
}

impl SkillsDirResolution {
    /// Build a user-facing warning when duplicate skill names exist in both
    /// `skills/` and `.skills/`.
    pub fn conflict_warning(&self) -> Option<String> {
        if self.conflicting_skill_names.is_empty() {
            return None;
        }
        Some(format!(
            "⚠ Duplicate skill names found in both skills/ and .skills/: {}. \
Skill resolution may be ambiguous; consider keeping only one copy.",
            self.conflicting_skill_names.join(", ")
        ))
    }
}

/// Resolve a skills directory path with default legacy fallback behavior:
/// when default `skills` (or `./skills`) does not exist but `.skills` exists,
/// fallback to `.skills`.
///
/// Also detects duplicate skill names between `skills/` and `.skills/` when
/// default-directory mode is used.
pub fn resolve_skills_dir_with_legacy_fallback(
    workspace: &Path,
    skills_dir: &str,
) -> SkillsDirResolution {
    let requested_path = resolve_path_in_workspace(workspace, skills_dir);
    let is_default = matches!(skills_dir, "skills" | "./skills");
    let legacy_path = workspace.join(".skills");

    let effective_path = if is_default && !requested_path.exists() && legacy_path.is_dir() {
        legacy_path.clone()
    } else {
        requested_path.clone()
    };

    let conflicting_skill_names = if is_default {
        find_duplicate_skill_names(&requested_path, &legacy_path)
    } else {
        Vec::new()
    };

    SkillsDirResolution {
        requested_path,
        used_legacy_fallback: effective_path == legacy_path,
        effective_path,
        conflicting_skill_names,
    }
}

fn resolve_path_in_workspace(workspace: &Path, input: &str) -> PathBuf {
    let p = PathBuf::from(input);
    if p.is_absolute() {
        p
    } else {
        workspace.join(p)
    }
}

fn find_duplicate_skill_names(primary: &Path, legacy: &Path) -> Vec<String> {
    if !primary.is_dir() || !legacy.is_dir() {
        return Vec::new();
    }
    let Ok(primary_real) = primary.canonicalize() else {
        return Vec::new();
    };
    let Ok(legacy_real) = legacy.canonicalize() else {
        return Vec::new();
    };
    if primary_real == legacy_real {
        return Vec::new();
    }
    let primary_names = collect_skill_names(primary);
    let legacy_names = collect_skill_names(legacy);
    let mut duplicates: Vec<String> = primary_names
        .intersection(&legacy_names)
        .map(|s| s.to_string())
        .collect();
    duplicates.sort();
    duplicates
}

fn collect_skill_names(root: &Path) -> HashSet<String> {
    let mut names = HashSet::new();
    let Ok(entries) = fs::read_dir(root) else {
        return names;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join("SKILL.md").exists() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            names.insert(name.to_string());
        }
    }
    names
}

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

/// Discover concrete skill directories visible from a workspace, including:
/// - regular skills from canonical search roots
/// - evolved skills under `<skills-root>/_evolved/*`
/// - pending evolved skills under `<skills-root>/_evolved/_pending/*`
///
/// Deduplicates by canonical path and returns entries sorted by path.
pub fn discover_skill_instances_in_workspace(
    workspace: &Path,
    search_dirs: Option<&[&str]>,
) -> Vec<SkillInstance> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for path in discover_skills_in_workspace(workspace, search_dirs) {
        push_skill_instance(&path, &mut seen, &mut result);
    }

    let parent_dirs = discover_skill_dirs_for_loading(workspace, search_dirs);
    for parent in parent_dirs {
        let parent = PathBuf::from(parent);
        collect_skill_instances_under(&parent.join("_evolved"), &mut seen, &mut result);
        collect_skill_instances_under(
            &parent.join("_evolved").join("_pending"),
            &mut seen,
            &mut result,
        );
    }

    result.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

fn collect_skill_instances_under(
    root: &Path,
    seen: &mut HashSet<PathBuf>,
    result: &mut Vec<SkillInstance>,
) {
    if !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut children: Vec<_> = entries.flatten().collect();
    children.sort_by_key(|e| e.file_name());
    for entry in children {
        push_skill_instance(&entry.path(), seen, result);
    }
}

fn push_skill_instance(path: &Path, seen: &mut HashSet<PathBuf>, result: &mut Vec<SkillInstance>) {
    if !path.is_dir() || !path.join("SKILL.md").exists() {
        return;
    }
    let Ok(real) = path.canonicalize() else {
        return;
    };
    if !seen.insert(real) {
        return;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    result.push(SkillInstance {
        name: name.to_string(),
        path: path.to_path_buf(),
    });
}

/// Discover skill directories for `load_skills`, as `Vec<String>`.
///
/// Returns parent dirs (e.g. `.skills`, `skills`) so `load_skills` can:
/// - scan subdirs for regular skills
/// - load evolved skills from `_evolved/` (EVO-4)
///   Using parent dirs ensures both regular and evolved skills are loaded.
pub fn discover_skill_dirs_for_loading(
    workspace: &Path,
    search_dirs: Option<&[&str]>,
) -> Vec<String> {
    let dirs = search_dirs.unwrap_or(SKILL_SEARCH_DIRS);
    let mut result = Vec::new();
    for d in dirs {
        let p = workspace.join(d);
        if p.is_dir() {
            result.push(p.to_string_lossy().to_string());
        }
    }
    result
}
