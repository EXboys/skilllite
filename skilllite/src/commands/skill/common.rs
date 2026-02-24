//! Shared helpers for skill management commands.

use anyhow::Result;
use skilllite_core::skill::manifest::{self, SkillIntegrityStatus};
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

            let (integrity_status, source, manifest_version, signature_status, installed_at) =
                integrity_json_fields(skill_path);
            let (trust_tier, trust_score, trust_reason) = trust_json_fields(skill_path);
            serde_json::json!({
                "name": name,
                "description": meta.description,
                "language": lang,
                "version": manifest_version.or(meta.version.clone()),
                "entry_point": if meta.entry_point.is_empty() { "" } else { meta.entry_point.as_str() },
                "network_enabled": meta.network.enabled,
                "compatibility": meta.compatibility,
                "resolved_packages": meta.resolved_packages,
                "allowed_tools": meta.allowed_tools,
                "integrity_status": integrity_status,
                "source": source,
                "signature_status": signature_status,
                "installed_at": installed_at,
                "trust_tier": trust_tier,
                "trust_score": trust_score,
                "trust_reason": trust_reason,
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

pub fn trust_tier_for_skill(skill_path: &Path) -> String {
    let Some(skills_dir) = skill_path.parent() else {
        return "UNKNOWN".to_string();
    };
    match manifest::evaluate_skill_status(skills_dir, skill_path) {
        Ok(report) => format!("{:?}", report.trust_tier).to_uppercase(),
        Err(_) => "UNKNOWN".to_string(),
    }
}

pub fn status_label_for_skill(skill_path: &Path) -> String {
    let Some(skills_dir) = skill_path.parent() else {
        return "UNSIGNED".to_string();
    };
    match manifest::evaluate_skill_status(skills_dir, skill_path) {
        Ok(report) => match report.status {
            SkillIntegrityStatus::Ok => "OK".to_string(),
            SkillIntegrityStatus::HashChanged => "HASH_CHANGED".to_string(),
            SkillIntegrityStatus::SignatureInvalid => "SIGNATURE_INVALID".to_string(),
            SkillIntegrityStatus::Unsigned => "UNSIGNED".to_string(),
        },
        Err(_) => "UNSIGNED".to_string(),
    }
}

fn integrity_json_fields(
    skill_path: &Path,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let Some(skills_dir) = skill_path.parent() else {
        return (
            "UNSIGNED".to_string(),
            None,
            None,
            Some("UNSIGNED".to_string()),
            None,
        );
    };

    let Ok(report) = manifest::evaluate_skill_status(skills_dir, skill_path) else {
        return (
            "UNSIGNED".to_string(),
            None,
            None,
            Some("UNSIGNED".to_string()),
            None,
        );
    };

    let status = match report.status {
        SkillIntegrityStatus::Ok => "OK".to_string(),
        SkillIntegrityStatus::HashChanged => "HASH_CHANGED".to_string(),
        SkillIntegrityStatus::SignatureInvalid => "SIGNATURE_INVALID".to_string(),
        SkillIntegrityStatus::Unsigned => "UNSIGNED".to_string(),
    };

    let signature = match report.signature_status {
        manifest::SignatureStatus::Unsigned => "UNSIGNED".to_string(),
        manifest::SignatureStatus::Valid => "VALID".to_string(),
        manifest::SignatureStatus::Invalid => "INVALID".to_string(),
    };

    let source = report.entry.as_ref().map(|e| e.source.clone());
    let version = report.entry.as_ref().and_then(|e| e.version.clone());
    let installed_at = report
        .entry
        .as_ref()
        .map(|e| e.installed_at.to_rfc3339());

    (status, source, version, Some(signature), installed_at)
}

fn trust_json_fields(skill_path: &Path) -> (String, u8, Vec<String>) {
    let Some(skills_dir) = skill_path.parent() else {
        return ("UNKNOWN".to_string(), 0, vec![]);
    };
    let Ok(report) = manifest::evaluate_skill_status(skills_dir, skill_path) else {
        return ("UNKNOWN".to_string(), 0, vec![]);
    };
    (
        format!("{:?}", report.trust_tier).to_uppercase(),
        report.trust_score,
        report.trust_reasons,
    )
}
