//! Skill discovery, directory copying, and dependency installation.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use skilllite_core::skill::metadata;

const SKILL_SEARCH_DIRS: &[&str] = &["skills", ".skills", ".agents/skills", ".claude/skills", "."];

pub(super) fn discover_skills(
    repo_dir: &Path,
    subpath: Option<&str>,
    skill_filter: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if repo_dir.join("SKILL.md").exists() {
        return vec![repo_dir.to_path_buf()];
    }

    if let Some(sp) = subpath {
        let target = repo_dir.join(sp);
        if target.is_dir() && target.join("SKILL.md").exists() {
            return vec![target];
        }
        if target.is_dir() {
            if let Ok(entries) = fs::read_dir(&target) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").exists() {
                        candidates.push(p);
                    }
                }
                if !candidates.is_empty() {
                    candidates.sort();
                    return candidates;
                }
            }
        }
        let skill_name = sp.split('/').last().unwrap_or(sp);
        for search_dir in SKILL_SEARCH_DIRS {
            if *search_dir == "." {
                continue;
            }
            let candidate = repo_dir.join(search_dir).join(skill_name);
            if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                candidates.push(candidate);
            }
        }
        if candidates.is_empty() && sp.contains('/') {
            for search_dir in SKILL_SEARCH_DIRS {
                if *search_dir == "." {
                    continue;
                }
                let candidate = repo_dir.join(search_dir).join(sp);
                if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                    candidates.push(candidate);
                }
            }
        }
        if candidates.is_empty() {
            find_skill_by_name_recursive(repo_dir, skill_name, &mut candidates);
        }
        return candidates;
    }

    let mut seen = std::collections::HashSet::new();
    for search_dir in SKILL_SEARCH_DIRS {
        let search_path = repo_dir.join(search_dir);
        if !search_path.is_dir() {
            continue;
        }
        if *search_dir == "." {
            if let Ok(entries) = fs::read_dir(&search_path) {
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
        } else {
            if search_path.join("SKILL.md").exists() {
                if let Ok(real) = search_path.canonicalize() {
                    if seen.insert(real) {
                        candidates.push(search_path.clone());
                    }
                }
            }
            if let Ok(entries) = fs::read_dir(&search_path) {
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
        }
    }

    if let Some(filter) = skill_filter {
        candidates.retain(|c| {
            c.file_name()
                .map(|n| n.to_string_lossy() == filter)
                .unwrap_or(false)
        });
    }

    candidates
}

fn find_skill_by_name_recursive(dir: &Path, name: &str, results: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let dir_name = p.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "__pycache__" {
                continue;
            }
            if dir_name == name && p.join("SKILL.md").exists() {
                results.push(p.clone());
            }
            find_skill_by_name_recursive(&p, name, results);
        }
    }
}

// ─── Skill Copy ─────────────────────────────────────────────────────────────

const COPY_EXCLUDE_DIRS: &[&str] = &[
    ".git", "__pycache__", "node_modules", "venv", ".venv", ".tox", ".mypy_cache", ".pytest_cache",
    ".ruff_cache", "dist", "build", "*.egg-info",
];

const COPY_EXCLUDE_FILES: &[&str] = &[".DS_Store", "Thumbs.db"];

const COPY_EXCLUDE_EXTENSIONS: &[&str] = &["pyc", "pyo"];

pub(super) fn copy_skill(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest)
            .with_context(|| format!("Failed to remove existing skill: {}", dest.display()))?;
    }
    copy_dir_filtered(src, dest)?;
    Ok(())
}

fn copy_dir_filtered(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory: {}", dest.display()))?;
    for entry in fs::read_dir(src)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if COPY_EXCLUDE_DIRS.iter().any(|d| {
            if d.contains('*') {
                let prefix = d.trim_end_matches('*').trim_end_matches('.');
                name_str.ends_with(prefix) || name_str.starts_with(prefix)
            } else {
                name_str == *d
            }
        }) && entry.path().is_dir()
        {
            continue;
        }

        if COPY_EXCLUDE_FILES.contains(&name_str.as_ref()) {
            continue;
        }

        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if COPY_EXCLUDE_EXTENSIONS.contains(&ext) {
                continue;
            }
        }

        let src_path = entry.path();
        let dest_path = dest.join(&name);
        if src_path.is_dir() {
            copy_dir_filtered(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .with_context(|| format!("Failed to copy: {}", src_path.display()))?;
        }
    }
    Ok(())
}

// ─── Dependency Installation ────────────────────────────────────────────────

pub(super) fn install_skill_deps(skills_dir: &Path, installed: &[String]) -> Vec<String> {
    let mut messages = Vec::new();
    for name in installed {
        let skill_path = skills_dir.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }
        match metadata::parse_skill_metadata(&skill_path) {
            Ok(meta) => {
                let cache_dir: Option<&str> = None;
                match skilllite_sandbox::env::builder::ensure_environment(&skill_path, &meta, cache_dir) {
                    Ok(_) => {
                        let lang = metadata::detect_language(&skill_path, &meta);
                        messages.push(format!(
                            "   ✓ {} [{}]: dependencies installed",
                            name, lang
                        ));
                    }
                    Err(e) => {
                        messages.push(format!("   ✗ {}: dependency error: {}", name, e));
                    }
                }
            }
            Err(e) => {
                messages.push(format!("   ✗ {}: parse error: {}", name, e));
            }
        }
    }
    messages
}
