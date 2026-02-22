//! `skilllite list` — List all installed skills.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use skilllite_core::skill::metadata;

use super::common;

/// `skilllite list`
pub fn cmd_list(skills_dir: &str, json_output: bool) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);

    if !skills_path.exists() {
        if json_output {
            println!("[]");
        } else {
            eprintln!("No skills directory found. Run `skilllite add` first.");
        }
        return Ok(());
    }

    let mut skill_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&skills_path) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                skill_dirs.push(p);
            }
        }
    }

    if skill_dirs.is_empty() {
        if json_output {
            println!("[]");
        } else {
            eprintln!("No skills installed.");
        }
        return Ok(());
    }

    if json_output {
        let mut skills_json = Vec::new();
        for skill_path in &skill_dirs {
            let info = common::skill_to_json(skill_path);
            skills_json.push(info);
        }
        println!("{}", serde_json::to_string_pretty(&skills_json)?);
        return Ok(());
    }

    eprintln!("📋 Installed skills ({}):", skill_dirs.len());
    eprintln!();
    for skill_path in &skill_dirs {
        match metadata::parse_skill_metadata(skill_path) {
            Ok(meta) => {
                let name = if meta.name.is_empty() {
                    skill_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                } else {
                    meta.name.clone()
                };
                let lang = metadata::detect_language(skill_path, &meta);
                let lang_tag = if lang != "unknown" {
                    format!("[{}]", lang)
                } else {
                    String::new()
                };
                eprintln!("  • {} {}", name, lang_tag);
                if let Some(ref desc) = meta.description {
                    let short: String = desc.chars().take(80).collect();
                    eprintln!("    {}", short);
                }
                eprintln!("    path: {}", skill_path.display());
            }
            Err(e) => {
                let name = skill_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                eprintln!("  • {}", name);
                eprintln!("    ⚠ Could not parse SKILL.md: {}", e);
            }
        }
        eprintln!();
    }

    Ok(())
}
