//! `skilllite remove` — Remove an installed skill.

use anyhow::Context;
use std::fs;

use skilllite_core::skill::manifest;
use skilllite_core::skill::metadata;

use super::common;

use crate::error::bail;
use crate::Result;

/// `skilllite remove <name>`
pub fn cmd_remove(skill_name: &str, skills_dir: &str, force: bool) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);

    if !skills_path.exists() {
        bail!("No skills directory found. Nothing to remove.");
    }

    let Some(skill_path) = locate_installed_skill(&skills_path, skill_name)? else {
        bail!(
            "Skill '{}' not found in {}",
            skill_name,
            skills_path.display()
        );
    };

    if !force {
        let dir_name = skill_path.file_name().unwrap_or_default().to_string_lossy();
        eprint!(
            "Remove skill '{}' from {}? [y/N] ",
            dir_name,
            skills_path.display()
        );
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    let dir_name = skill_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    fs::remove_dir_all(&skill_path)
        .with_context(|| format!("Failed to remove skill: {}", skill_path.display()))?;
    let _ = manifest::remove_skill_entry(&skills_path, &skill_path);
    eprintln!("✓ Removed skill '{}'", dir_name);
    Ok(())
}

/// Resolve a skill directory that is a direct child of `skills_path`.
///
/// A name that is not a single directory component is never joined onto
/// `skills_path`. It can only match an existing child's metadata name, so
/// `../../.config` cannot delete a directory outside the skills root.
fn locate_installed_skill(
    skills_path: &std::path::Path,
    skill_name: &str,
) -> Result<Option<std::path::PathBuf>> {
    if skilllite_core::skill::validate_skill_dir_name(skill_name).is_ok() {
        let direct = skills_path.join(skill_name);
        if direct.is_dir() {
            return Ok(Some(direct));
        }
    }

    let entries = match fs::read_dir(skills_path) {
        Ok(entries) => entries,
        Err(err) => {
            return Err(crate::Error::validation(format!(
                "Failed to read skills directory {}: {err}",
                skills_path.display()
            )));
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join("SKILL.md").exists() {
            continue;
        }
        if entry.file_name().to_string_lossy() == skill_name {
            return Ok(Some(path));
        }
        if let Ok(meta) = metadata::parse_skill_metadata(&path) {
            if meta.name == skill_name {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_traversal_name_does_not_delete_outside_skills_dir() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("project").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        let victim = root.path().join(".config");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("keep.txt"), "保留").unwrap();

        let err = cmd_remove("../../.config", skills.to_str().unwrap(), true).unwrap_err();
        assert!(
            err.to_string().contains("not found") || err.to_string().contains("directory name"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(victim.join("keep.txt")).unwrap(),
            "保留"
        );
        assert!(skills.is_dir());
    }

    #[test]
    fn remove_metadata_traversal_name_deletes_only_the_skill_directory() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("project").join("skills");
        let skill = skills.join("notes");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: ../../.config\ndescription: notes\n---\n# Notes\n",
        )
        .unwrap();
        let victim = root.path().join(".config");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("keep.txt"), "保留").unwrap();

        cmd_remove("../../.config", skills.to_str().unwrap(), true).unwrap();

        assert!(!skill.exists());
        assert_eq!(
            std::fs::read_to_string(victim.join("keep.txt")).unwrap(),
            "保留"
        );
    }

    #[test]
    fn remove_plain_name_still_deletes_that_skill() {
        let root = tempfile::tempdir().unwrap();
        let skills = root.path().join("skills");
        let skill = skills.join("计算器");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: 计算器\n---\n").unwrap();

        cmd_remove("计算器", skills.to_str().unwrap(), true).unwrap();

        assert!(!skill.exists());
        assert!(skills.is_dir());
    }
}
