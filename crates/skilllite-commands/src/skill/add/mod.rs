//! `skilllite add` — Add skills from remote repo, ClawHub, or local path.

use anyhow::Context;
use std::fs;
use std::path::PathBuf;

use skilllite_core::skill::manifest;
use skilllite_core::skill::metadata;

use super::common;

use crate::error::bail;
use crate::Result;

mod admission;
mod discovery;
mod source;

pub(in crate::skill) use admission::{
    scan_candidate_skills, scan_candidate_skills_fast, AdmissionRisk,
};
pub(in crate::skill) use discovery::{copy_skill, install_skill_deps};

use discovery::discover_skills;
use source::{clone_repo, fetch_from_clawhub, parse_source};

pub fn cmd_add(
    source: &str,
    skills_dir: &str,
    force: bool,
    list_only: bool,
    scan_offline: bool,
) -> Result<()> {
    let skills_path = common::resolve_skills_dir(skills_dir);
    let parsed = parse_source(source);

    eprintln!("📦 Source: {}", source);
    eprintln!("   Type: {}", parsed.source_type);
    eprintln!("   URL: {}", parsed.url);
    if let Some(ref r) = parsed.git_ref {
        eprintln!("   Ref: {}", r);
    }
    if let Some(ref sp) = parsed.subpath {
        eprintln!("   Subpath: {}", sp);
    }
    if let Some(ref f) = parsed.skill_filter {
        eprintln!("   Filter: {}", f);
    }
    eprintln!();

    let mut temp_dir: Option<PathBuf> = None;
    let result = (|| -> Result<()> {
        let repo_dir = if parsed.source_type == "local" {
            let p = PathBuf::from(&parsed.url);
            if !p.is_dir() {
                bail!("Local path does not exist: {}", parsed.url);
            }
            eprintln!("📁 Using local path: {}", parsed.url);
            p
        } else if parsed.source_type == "clawhub" {
            eprintln!("⬇ Downloading from ClawHub ({}) ...", parsed.url);
            let td = fetch_from_clawhub(&parsed.url)?;
            eprintln!("✓ Download complete");
            temp_dir = Some(td.clone());
            td
        } else {
            eprintln!("⬇ Cloning {} ...", parsed.url);
            let td = clone_repo(&parsed.url, parsed.git_ref.as_deref())?;
            eprintln!("✓ Clone complete");
            temp_dir = Some(td.clone());
            td
        };

        eprintln!();
        eprintln!("🔍 Discovering skills...");
        let skills = discover_skills(
            &repo_dir,
            parsed.subpath.as_deref(),
            parsed.skill_filter.as_deref(),
        );

        if skills.is_empty() {
            eprintln!("   No skills found (no SKILL.md files detected)");
            bail!("No skills found");
        }

        eprintln!("   Found {} skill(s):", skills.len());
        for s in &skills {
            match metadata::parse_skill_metadata(s) {
                Ok(meta) => {
                    let desc = meta.description.as_deref().unwrap_or("");
                    let short_desc: String = desc.chars().take(60).collect();
                    eprintln!("   • {}: {}", meta.name, short_desc);
                }
                Err(_) => {
                    let name = s.file_name().unwrap_or_default().to_string_lossy();
                    eprintln!("   • {}: (could not parse SKILL.md)", name);
                }
            }
        }

        if list_only {
            return Ok(());
        }

        eprintln!();
        fs::create_dir_all(&skills_path).context("Failed to create skills directory")?;

        let mut install_candidates: Vec<(String, PathBuf)> = Vec::new();
        for skill_path in &skills {
            let skill_name = match metadata::parse_skill_metadata(skill_path) {
                Ok(meta) if !meta.name.is_empty() => meta.name,
                _ => skill_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            };

            let dest = skills_path.join(&skill_name);
            if dest.exists() && !force {
                eprintln!(
                    "   ⏭ {}: already exists (use --force to overwrite)",
                    skill_name
                );
                continue;
            }
            install_candidates.push((skill_name, skill_path.clone()));
        }

        if install_candidates.is_empty() {
            eprintln!("   No new skills installed.");
            return Ok(());
        }

        eprintln!();
        if scan_offline {
            eprintln!("🔍 Running admission scans (offline: local rules only, no LLM/network)...");
        } else {
            eprintln!("🔍 Running admission scans (content-based)...");
        }
        let scan_reports = scan_candidate_skills(&install_candidates, scan_offline);
        let mut malicious = Vec::new();
        let mut suspicious = Vec::new();
        for report in &scan_reports {
            eprintln!("   ▶ {} => {}", report.name, report.risk.as_str());
            for msg in &report.messages {
                eprintln!("{}", msg);
            }
            match report.risk {
                AdmissionRisk::Malicious => malicious.push(report.name.clone()),
                AdmissionRisk::Suspicious => suspicious.push(report.name.clone()),
                AdmissionRisk::Safe => {}
            }
        }
        if !malicious.is_empty() {
            eprintln!("   ❌ Blocked malicious skill(s): {}", malicious.join(", "));
        }
        if !suspicious.is_empty() && !force {
            eprintln!(
                "   ⚠️  Skipped suspicious skill(s): {} (use --force to install)",
                suspicious.join(", ")
            );
        }
        if !suspicious.is_empty() && force {
            eprintln!(
                "   ⚠️  Continuing with --force for suspicious skills: {}",
                suspicious.join(", ")
            );
        }

        let risk_by_name: std::collections::HashMap<String, &str> = scan_reports
            .iter()
            .map(|r| (r.name.clone(), r.risk.as_str()))
            .collect();
        let blocked: std::collections::HashSet<&str> =
            malicious.iter().map(|s| s.as_str()).collect();
        let skipped_suspicious: std::collections::HashSet<&str> = if force {
            std::collections::HashSet::new()
        } else {
            suspicious.iter().map(|s| s.as_str()).collect()
        };
        let mut installed: Vec<String> = Vec::new();
        for (skill_name, skill_path) in &install_candidates {
            if blocked.contains(skill_name.as_str()) {
                continue;
            }
            if skipped_suspicious.contains(skill_name.as_str()) {
                continue;
            }
            let dest = skills_path.join(skill_name);
            copy_skill(skill_path, &dest)?;
            let admission = risk_by_name.get(skill_name).copied();
            let _entry = manifest::upsert_installed_skill_with_admission(
                &skills_path,
                &dest,
                source,
                admission,
            )?;
            installed.push(skill_name.clone());
            eprintln!("   ✓ {}: installed to {}", skill_name, dest.display());
        }

        if installed.is_empty() {
            eprintln!("   No new skills installed.");
            return Ok(());
        }

        eprintln!();
        eprintln!("📦 Installing dependencies...");
        let dep_messages = install_skill_deps(&skills_path, &installed);
        for msg in &dep_messages {
            eprintln!("{}", msg);
        }

        eprintln!();
        eprintln!("{}", "=".repeat(50));
        eprintln!(
            "🎉 Successfully added {} skill(s) from {}",
            installed.len(),
            source
        );
        for name in &installed {
            eprintln!("  • {}", name);
        }
        eprintln!("{}", "=".repeat(50));

        Ok(())
    })();

    if let Some(ref td) = temp_dir {
        let _ = fs::remove_dir_all(td);
    }

    result
}

/// 从源头更新单个技能（用于 repair：下载的技能失败时按源头覆盖）
pub fn update_skill_from_source(
    skills_path: &std::path::Path,
    skill_name: &str,
    source: &str,
) -> Result<()> {
    let parsed = parse_source(source);
    let mut temp_dir: Option<PathBuf> = None;

    let repo_dir = if parsed.source_type == "local" {
        let p = PathBuf::from(&parsed.url);
        if !p.is_dir() {
            bail!("Local path does not exist: {}", parsed.url);
        }
        p
    } else if parsed.source_type == "clawhub" {
        let td = fetch_from_clawhub(&parsed.url)?;
        temp_dir = Some(td.clone());
        td
    } else {
        let td = clone_repo(&parsed.url, parsed.git_ref.as_deref())?;
        temp_dir = Some(td.clone());
        td
    };

    let skills = discover_skills(&repo_dir, parsed.subpath.as_deref(), Some(skill_name));

    let skill_path = skills
        .into_iter()
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy() == skill_name)
                .unwrap_or(false)
        })
        .ok_or_else(|| crate::Error::validation(format!("源头中未找到技能: {}", skill_name)))?;

    let dest = skills_path.join(skill_name);
    copy_skill(&skill_path, &dest)?;
    manifest::upsert_installed_skill(skills_path, &dest, source)?;

    if let Some(ref td) = temp_dir {
        let _ = fs::remove_dir_all(td);
    }
    Ok(())
}
