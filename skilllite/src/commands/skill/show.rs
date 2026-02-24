//! `skilllite show` — Show detailed information about a skill.

use anyhow::Result;

use skilllite_core::skill::metadata;

use super::common;

/// `skilllite show <name>`
pub fn cmd_show(skill_name: &str, skills_dir: &str, json_output: bool) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);
    let skill_path = common::find_skill(&skills_path, skill_name)?;
    let meta = metadata::parse_skill_metadata(&skill_path)?;
    let lang = metadata::detect_language(&skill_path, &meta);

    if json_output {
        let info = common::skill_to_json(&skill_path);
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    eprintln!("📦 Skill: {}", meta.name);
    eprintln!("   Path: {}", skill_path.display());
    eprintln!("   Integrity: {}", common::status_label_for_skill(&skill_path));
    if let Some(ref desc) = meta.description {
        eprintln!("   Description: {}", desc);
    }
    eprintln!("   Language: {}", lang);
    if meta.entry_point.is_empty() {
        if meta.is_bash_tool_skill() {
            eprintln!("   Type: bash-tool skill");
            if let Some(ref at) = meta.allowed_tools {
                eprintln!("   Allowed Tools: {}", at);
            }
        } else {
            eprintln!("   Type: prompt-only skill");
        }
    } else {
        eprintln!("   Entry Point: {}", meta.entry_point);
    }
    eprintln!(
        "   Network: {}",
        if meta.network.enabled { "enabled" } else { "disabled" }
    );
    if !meta.network.outbound.is_empty() {
        eprintln!("   Outbound: {}", meta.network.outbound.join(", "));
    }
    if let Some(ref compat) = meta.compatibility {
        eprintln!("   Compatibility: {}", compat);
    }
    if let Some(ref pkgs) = meta.resolved_packages {
        eprintln!("   Resolved Packages: {}", pkgs.join(", "));
    }

    let scripts_dir = skill_path.join("scripts");
    if scripts_dir.is_dir() {
        eprintln!("   Scripts:");
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    eprintln!("     - {}", name);
                }
            }
        }
    }

    let refs_dir = skill_path.join("references");
    if refs_dir.is_dir() {
        eprintln!("   References:");
        if let Ok(entries) = std::fs::read_dir(&refs_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') {
                    eprintln!("     - {}", name);
                }
            }
        }
    }

    Ok(())
}
