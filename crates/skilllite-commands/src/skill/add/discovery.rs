//! Skill discovery, directory copying, and dependency installation.

use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;

use skilllite_core::skill::discovery::SKILL_SEARCH_DIRS;
use skilllite_core::skill::metadata;

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
        let skill_name = sp.split('/').next_back().unwrap_or(sp);
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

    let mut candidates =
        skilllite_core::skill::discovery::discover_skills_in_workspace(repo_dir, None);

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
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "__pycache__"
            {
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
    ".git",
    "__pycache__",
    "node_modules",
    "venv",
    ".venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "dist",
    "build",
    "*.egg-info",
];

const COPY_EXCLUDE_FILES: &[&str] = &[".DS_Store", "Thumbs.db"];

const COPY_EXCLUDE_EXTENSIONS: &[&str] = &["pyc", "pyo"];

pub(in crate::skill) fn copy_skill(src: &Path, dest: &Path) -> Result<()> {
    // Inspect first: copy currently deletes dest, so a later symlink reject
    // must not wipe an already-installed skill.
    reject_copied_symlinks(src)?;
    if dest.exists() {
        fs::remove_dir_all(dest)
            .with_context(|| format!("Failed to remove existing skill: {}", dest.display()))?;
    }
    copy_dir_filtered(src, dest)?;
    Ok(())
}

fn is_excluded_dir_name(name_str: &str) -> bool {
    COPY_EXCLUDE_DIRS.iter().any(|d| {
        if d.contains('*') {
            let prefix = d.trim_end_matches('*').trim_end_matches('.');
            name_str.ends_with(prefix) || name_str.starts_with(prefix)
        } else {
            name_str == *d
        }
    })
}

fn is_excluded_file(name_str: &str, path: &Path) -> bool {
    if COPY_EXCLUDE_FILES.contains(&name_str) {
        return true;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        return COPY_EXCLUDE_EXTENSIONS.contains(&ext);
    }
    false
}

fn reject_copied_symlinks(src: &Path) -> Result<()> {
    inspect_or_copy_dir(src, None)
}

fn copy_dir_filtered(src: &Path, dest: &Path) -> Result<()> {
    inspect_or_copy_dir(src, Some(dest))
}

/// Walk `src`. When `dest` is `Some`, copy regular files/dirs into it.
/// Fail closed on any symlink that would otherwise be copied. Excluded
/// directory names (including when they are themselves symlinks) are skipped.
fn inspect_or_copy_dir(src: &Path, dest: Option<&Path>) -> Result<()> {
    if let Some(dest) = dest {
        fs::create_dir_all(dest)
            .with_context(|| format!("Failed to create directory: {}", dest.display()))?;
    }
    for entry in fs::read_dir(src)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let src_path = entry.path();
        let meta = fs::symlink_metadata(&src_path)
            .with_context(|| format!("Failed to inspect: {}", src_path.display()))?;
        let is_symlink = meta.file_type().is_symlink();
        let looks_like_dir = meta.is_dir() || (is_symlink && src_path.is_dir());

        if is_excluded_dir_name(name_str.as_ref()) && looks_like_dir {
            continue;
        }
        if is_excluded_file(name_str.as_ref(), &src_path) {
            continue;
        }
        if is_symlink {
            crate::error::bail!(
                "Refusing to install skill: symlink is not allowed ({})",
                src_path.display()
            );
        }

        if let Some(dest) = dest {
            let dest_path = dest.join(&name);
            if meta.is_dir() {
                inspect_or_copy_dir(&src_path, Some(&dest_path))?;
            } else {
                fs::copy(&src_path, &dest_path)
                    .with_context(|| format!("Failed to copy: {}", src_path.display()))?;
            }
        } else if meta.is_dir() {
            inspect_or_copy_dir(&src_path, None)?;
        }
    }
    Ok(())
}

// ─── Dependency Installation ────────────────────────────────────────────────

pub(in crate::skill) fn install_skill_deps(skills_dir: &Path, installed: &[String]) -> Vec<String> {
    let mut messages = Vec::new();
    for name in installed {
        let skill_path = skills_dir.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }
        match metadata::parse_skill_metadata(&skill_path) {
            Ok(meta) => {
                let cache_dir: Option<&str> = None;
                let env_spec = skilllite_core::EnvSpec::from_metadata(&skill_path, &meta);
                match skilllite_sandbox::env::builder::ensure_environment(
                    &skill_path,
                    &env_spec,
                    cache_dir,
                    None,
                    skilllite_sandbox::cli_confirm_download(),
                ) {
                    Ok(_) => {
                        let lang = &env_spec.language;
                        messages.push(format!("   ✓ {} [{}]: dependencies installed", name, lang));
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

#[cfg(test)]
mod tests {
    use super::copy_skill;
    use std::fs;
    use std::path::Path;

    fn write_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).expect("skill dir");
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n"),
        )
        .expect("skill md");
    }

    #[test]
    fn copy_skill_copies_regular_tree() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("src");
        write_skill(&src, "plain");
        fs::write(src.join("notes.txt"), "hello").expect("notes");
        let dest = tmp.path().join("dest");
        copy_skill(&src, &dest).expect("copy regular skill");
        assert_eq!(fs::read_to_string(dest.join("notes.txt")).unwrap(), "hello");
        assert!(dest.join("SKILL.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn copy_skill_rejects_file_symlink_to_host_secret() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("src");
        write_skill(&src, "localsym");
        std::os::unix::fs::symlink("/etc/passwd", src.join("passwd-link")).expect("symlink");

        let dest = tmp.path().join("dest");
        fs::create_dir_all(&dest).expect("existing dest");
        fs::write(dest.join("KEEP.txt"), "keep-me").expect("marker");

        let err = copy_skill(&src, &dest).expect_err("symlink skill must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("symlink is not allowed"),
            "unexpected error: {msg}"
        );
        assert!(
            !dest.join("passwd-link").exists(),
            "must not materialize host file"
        );
        assert_eq!(
            fs::read_to_string(dest.join("KEEP.txt")).unwrap(),
            "keep-me",
            "existing dest must survive a rejected install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_skill_rejects_directory_symlink() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("src");
        write_skill(&src, "dirsym");
        std::os::unix::fs::symlink("/etc", src.join("etc-link")).expect("dir symlink");

        let dest = tmp.path().join("dest");
        let err = copy_skill(&src, &dest).expect_err("dir symlink must be rejected");
        assert!(err.to_string().contains("symlink is not allowed"));
        assert!(!dest.join("etc-link").exists());
    }

    #[cfg(unix)]
    #[test]
    fn copy_skill_skips_excluded_dir_symlink() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("src");
        write_skill(&src, "venv-link");
        std::os::unix::fs::symlink("/etc", src.join(".venv")).expect(".venv symlink");
        fs::write(src.join("ok.txt"), "ok").expect("ok");

        let dest = tmp.path().join("dest");
        copy_skill(&src, &dest).expect("excluded dir symlink should not fail install");
        assert!(dest.join("ok.txt").is_file());
        assert!(!dest.join(".venv").exists());
    }

    #[cfg(unix)]
    #[test]
    fn copy_skill_rejects_nested_symlink() {
        let tmp = tempfile::tempdir().expect("tmp");
        let src = tmp.path().join("src");
        write_skill(&src, "nested");
        let scripts = src.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts");
        std::os::unix::fs::symlink("/etc/passwd", scripts.join("secret")).expect("nested symlink");

        let dest = tmp.path().join("dest");
        let err = copy_skill(&src, &dest).expect_err("nested symlink must be rejected");
        assert!(err.to_string().contains("symlink is not allowed"));
        assert!(!dest.join("scripts/secret").exists());
    }
}
