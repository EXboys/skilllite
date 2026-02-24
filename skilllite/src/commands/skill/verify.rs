//! `skilllite verify` — Verify skill integrity (fingerprint/signature).

use anyhow::Result;
use std::path::PathBuf;

use skilllite_core::skill::manifest::{self, SignatureStatus, SkillIntegrityStatus};

use super::common;

/// `skilllite verify <name-or-path>`
pub fn cmd_verify(target: &str, skills_dir: &str, json_output: bool, strict: bool) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);
    let skill_path = resolve_target_path(target, &skills_path)?;
    let report = manifest::evaluate_skill_status(&skills_path, &skill_path)?;

    let status = match report.status {
        SkillIntegrityStatus::Ok => "OK",
        SkillIntegrityStatus::HashChanged => "HASH_CHANGED",
        SkillIntegrityStatus::SignatureInvalid => "SIGNATURE_INVALID",
        SkillIntegrityStatus::Unsigned => "UNSIGNED",
    };
    let signature = match report.signature_status {
        SignatureStatus::Unsigned => "UNSIGNED",
        SignatureStatus::Valid => "VALID",
        SignatureStatus::Invalid => "INVALID",
    };
    let manifest_hash = report
        .entry
        .as_ref()
        .map(|e| e.hash.clone())
        .unwrap_or_default();
    let source = report.entry.as_ref().map(|e| e.source.clone());
    let installed_at = report.entry.as_ref().map(|e| e.installed_at.to_rfc3339());

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "target": target,
                "path": skill_path.to_string_lossy(),
                "status": status,
                "signature_status": signature,
                "current_hash": report.current_hash,
                "manifest_hash": manifest_hash,
                "source": source,
                "installed_at": installed_at
            }))?
        );
    } else {
        eprintln!("🔎 Verify: {}", target);
        eprintln!("   Path: {}", skill_path.display());
        eprintln!("   Status: {}", status);
        eprintln!("   Signature: {}", signature);
        eprintln!("   Current Hash: {}", report.current_hash);
        if !manifest_hash.is_empty() {
            eprintln!("   Manifest Hash: {}", manifest_hash);
        }
        if let Some(ref src) = source {
            eprintln!("   Source: {}", src);
        }
        if let Some(ref at) = installed_at {
            eprintln!("   Installed At: {}", at);
        }
    }

    if strict {
        match report.status {
            SkillIntegrityStatus::Ok | SkillIntegrityStatus::Unsigned => {}
            SkillIntegrityStatus::HashChanged | SkillIntegrityStatus::SignatureInvalid => {
                anyhow::bail!("Strict verify failed: {}", status);
            }
        }
    }

    Ok(())
}

fn resolve_target_path(target: &str, skills_path: &std::path::Path) -> Result<PathBuf> {
    let input = PathBuf::from(target);
    if input.exists() {
        let p = if input.is_absolute() {
            input
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(input)
        };
        if p.is_dir() && p.join("SKILL.md").exists() {
            return Ok(p);
        }
    }

    common::find_skill(skills_path, target)
}
