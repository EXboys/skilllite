//! Sync skills shipped under Tauri `resources/bundled-skills/.skills/` into the workspace `.skills/`.
//!
//! - Empty `.skills` (missing, or no skill subdirs with SKILL.md): create and copy all bundled skills.
//! - Otherwise: merge — copy missing skills; replace existing only when bundled `metadata.version` is
//!   greater than the workspace copy (semver-like, loose parse). Bundled skill without version never
//!   overwrites an existing directory.

use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;
use skilllite_core::skill::metadata::parse_skill_metadata;
use tauri::{AppHandle, Manager};

/// `resource_dir()/bundled-skills/.skills/` — populated by `scripts/prebuild-skilllite.sh`.
fn bundled_skills_container(app: &AppHandle) -> Option<PathBuf> {
    let root = app.path().resource_dir().ok()?;
    let p = root.join("bundled-skills").join(".skills");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let fp = entry.path();
        let name = entry.file_name();
        if fp.is_dir() {
            copy_dir_recursive(&fp, &dst.join(name))?;
        } else {
            fs::copy(&fp, dst.join(name))?;
        }
    }
    Ok(())
}

fn is_reserved_skills_subdir(name: &str) -> bool {
    name.starts_with('_') || name.starts_with('.')
}

fn list_bundled_skill_dirs(container: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(container) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.join("SKILL.md").is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| !is_reserved_skills_subdir(n))
                    .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

/// True if `skills_root` has at least one immediate child directory (non-reserved) containing SKILL.md.
fn skills_dir_has_any_skill(skills_root: &Path) -> bool {
    if !skills_root.is_dir() {
        return false;
    }
    let Ok(entries) = fs::read_dir(skills_root) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        if !p.is_dir() {
            return false;
        }
        let name = e.file_name();
        let Some(ns) = name.to_str() else {
            return false;
        };
        if is_reserved_skills_subdir(ns) {
            return false;
        }
        p.join("SKILL.md").is_file()
    })
}

fn read_declared_version(skill_dir: &Path) -> Option<String> {
    parse_skill_metadata(skill_dir).ok().and_then(|m| m.version)
}

/// Parse a loose X.Y.Z from the start of the string (ignores pre-release suffixes for comparison).
fn parse_semver_loose(s: &str) -> Option<Version> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let core: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if core.is_empty() {
        return None;
    }
    let mut parts: Vec<u64> = core
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    while parts.len() < 3 {
        parts.push(0);
    }
    parts.truncate(3);
    Some(Version::new(parts[0], parts[1], parts[2]))
}

/// Replace existing workspace skill only when bundled version parses and is strictly greater.
fn bundled_is_newer_than_workspace(bundled_ver: &Option<String>, workspace_ver: &Option<String>) -> bool {
    let Some(b_raw) = bundled_ver else {
        return false;
    };
    let Some(bv) = parse_semver_loose(b_raw) else {
        return false;
    };
    let wv = workspace_ver
        .as_ref()
        .and_then(|s| parse_semver_loose(s))
        .unwrap_or_else(|| Version::new(0, 0, 0));
    bv > wv
}

fn replace_skill_dir(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|e| format!("remove {}: {}", dst.display(), e))?;
    }
    copy_dir_recursive(src, dst).map_err(|e| format!("copy {} -> {}: {}", src.display(), dst.display(), e))?;
    Ok(())
}

/// Sync bundled skills into `workspace/.skills/`. No-op if no bundled container exists.
pub fn sync_bundled_skills_from_resources(app: &AppHandle, workspace_raw: &str) -> Result<(), String> {
    let Some(bundled_root) = bundled_skills_container(app) else {
        return Ok(());
    };
    let bundled_skills = list_bundled_skill_dirs(&bundled_root);
    if bundled_skills.is_empty() {
        return Ok(());
    }

    let skills_target = super::paths::find_project_root(workspace_raw).join(".skills");
    fs::create_dir_all(&skills_target).map_err(|e| format!("create .skills: {}", e))?;

    let empty_or_missing = !skills_dir_has_any_skill(&skills_target);

    if empty_or_missing {
        for src in &bundled_skills {
            let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let dst = skills_target.join(name);
            replace_skill_dir(src, &dst)?;
        }
        eprintln!(
            "[skilllite-assistant] bundled skills: seeded {} skill(s) into {}",
            bundled_skills.len(),
            skills_target.display()
        );
        return Ok(());
    }

    let mut installed = 0usize;
    let mut upgraded = 0usize;
    let mut skipped = 0usize;

    for src in &bundled_skills {
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let dst = skills_target.join(name);
        let bundled_ver = read_declared_version(src);

        if !dst.exists() {
            copy_dir_recursive(&src, &dst).map_err(|e| {
                format!(
                    "copy bundled skill {} -> {}: {}",
                    src.display(),
                    dst.display(),
                    e
                )
            })?;
            installed += 1;
            continue;
        }

        let ws_ver = read_declared_version(&dst);
        if bundled_is_newer_than_workspace(&bundled_ver, &ws_ver) {
            replace_skill_dir(src, &dst)?;
            upgraded += 1;
        } else {
            skipped += 1;
        }
    }

    if installed > 0 || upgraded > 0 {
        eprintln!(
            "[skilllite-assistant] bundled skills: merged into {} (installed {}, upgraded {}, skipped {})",
            skills_target.display(),
            installed,
            upgraded,
            skipped
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_loose_orders() {
        assert!(parse_semver_loose("1.0.1").unwrap() > parse_semver_loose("1.0.0").unwrap());
        assert!(parse_semver_loose("1.1").unwrap() > parse_semver_loose("1.0.9").unwrap());
        assert_eq!(
            parse_semver_loose("1.0").unwrap(),
            parse_semver_loose("1.0.0").unwrap()
        );
    }

    #[test]
    fn replace_only_when_bundled_newer() {
        assert!(bundled_is_newer_than_workspace(
            &Some("2.0.0".into()),
            &Some("1.0.0".into())
        ));
        assert!(!bundled_is_newer_than_workspace(
            &Some("1.0.0".into()),
            &Some("2.0.0".into())
        ));
        assert!(!bundled_is_newer_than_workspace(
            &Some("1.0.0".into()),
            &Some("1.0.0".into())
        ));
        assert!(bundled_is_newer_than_workspace(&Some("1.0.0".into()), &None));
        assert!(!bundled_is_newer_than_workspace(&None, &Some("1.0.0".into())));
    }
}
