//! Shared helpers for skill management commands.

use anyhow::Result;
use skilllite_core::skill::metadata;
use std::path::{Path, PathBuf};

pub fn resolve_skills_dir(skills_dir: &str) -> PathBuf {
    let p = PathBuf::from(skills_dir);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub fn find_skill(skills_path: &Path, skill_name: &str) -> Result<PathBuf> {
    if !skills_path.exists() {
        anyhow::bail!("Skills directory not found: {}", skills_path.display());
    }

    let direct = skills_path.join(skill_name);
    if direct.is_dir() && direct.join("SKILL.md").exists() {
        return Ok(direct);
    }

    if let Ok(entries) = std::fs::read_dir(skills_path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() || !p.join("SKILL.md").exists() {
                continue;
            }
            if let Ok(meta) = metadata::parse_skill_metadata(&p) {
                if meta.name == skill_name {
                    return Ok(p);
                }
            }
        }
    }

    anyhow::bail!(
        "Skill '{}' not found in {}",
        skill_name,
        skills_path.display()
    )
}

pub fn skill_to_json(skill_path: &Path) -> serde_json::Value {
    match metadata::parse_skill_metadata(skill_path) {
        Ok(meta) => {
            let lang = metadata::detect_language(skill_path, &meta);
            let name = if meta.name.is_empty() {
                skill_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                meta.name.clone()
            };

            let multi_script_tools = if meta.entry_point.is_empty() && !meta.is_bash_tool_skill() {
                let tools =
                    skilllite_core::skill::schema::detect_multi_script_tools(skill_path, &name);
                tools
                    .into_iter()
                    .map(|t| {
                        serde_json::json!({
                            "tool_name": t.tool_name,
                            "skill_name": t.skill_name,
                            "script_path": t.script_path,
                            "language": t.language,
                            "input_schema": t.input_schema,
                            "description": t.description,
                        })
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };

            serde_json::json!({
                "name": name,
                "description": meta.description,
                "language": lang,
                "entry_point": if meta.entry_point.is_empty() { "" } else { meta.entry_point.as_str() },
                "network_enabled": meta.network.enabled,
                "compatibility": meta.compatibility,
                "resolved_packages": meta.resolved_packages,
                "allowed_tools": meta.allowed_tools,
                "path": skill_path.to_string_lossy(),
                "is_bash_tool": meta.is_bash_tool_skill(),
                "requires_elevated_permissions": meta.requires_elevated_permissions,
                "multi_script_tools": multi_script_tools,
            })
        }
        Err(e) => {
            let name = skill_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            serde_json::json!({
                "name": name,
                "error": e.to_string(),
                "path": skill_path.to_string_lossy(),
            })
        }
    }
}
