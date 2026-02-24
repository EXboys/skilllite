//! `skilllite remove` — Remove an installed skill.

use anyhow::{Context, Result};
use std::fs;

use skilllite_core::skill::metadata;
use skilllite_core::skill::manifest;

use super::common;

/// `skilllite remove <name>`
pub fn cmd_remove(skill_name: &str, skills_dir: &str, force: bool) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);

    if !skills_path.exists() {
        anyhow::bail!("No skills directory found. Nothing to remove.");
    }

    let mut skill_path = skills_path.join(skill_name);

    if !skill_path.exists() {
        let mut found = false;
        if let Ok(entries) = fs::read_dir(&skills_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() || !p.join("SKILL.md").exists() {
                    continue;
                }
                if let Ok(meta) = metadata::parse_skill_metadata(&p) {
                    if meta.name == skill_name {
                        skill_path = p;
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            anyhow::bail!(
                "Skill '{}' not found in {}",
                skill_name,
                skills_path.display()
            );
        }
    }

    if !force {
        let dir_name = skill_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
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
