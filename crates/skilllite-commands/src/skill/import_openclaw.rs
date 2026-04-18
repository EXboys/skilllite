//! Import skills from OpenClaw-style directory layout into a SkillLite `skills/` tree.
//!
//! Source precedence (first wins for duplicate skill names) matches common OpenClaw/Hermes docs:
//! workspace `*/skills`, workspace `*/.agents/skills`, `~/.openclaw/skills`, legacy bot dirs,
//! then `~/.agents/skills`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use skilllite_core::skill::manifest;
use skilllite_core::skill::metadata;

use super::add::{copy_skill, install_skill_deps, scan_candidate_skills, AdmissionRisk};
use super::common::resolve_skills_dir;

use crate::error::bail;
use crate::Result;

/// When the destination skills directory already contains a skill with the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillConflictPolicy {
    Skip,
    Overwrite,
    Rename,
}

impl SkillConflictPolicy {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "overwrite" => Ok(Self::Overwrite),
            "rename" => Ok(Self::Rename),
            _ => bail!(
                "Invalid --skill-conflict '{}': expected skip, overwrite, or rename",
                s
            ),
        }
    }
}

/// OpenClaw workspace folders next to a project (`workspace`, `workspace-main`, `workspace-*`, …).
pub fn openclaw_workspace_candidates(project_root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for name in ["workspace", "workspace-main", "workspace.default"] {
        let p = project_root.join(name);
        if p.is_dir() {
            out.push(p);
        }
    }
    let mut extra: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = fs::read_dir(project_root) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let file_name = e.file_name();
            let ns = file_name.to_string_lossy();
            if ns.starts_with("workspace-")
                && ns.as_ref() != "workspace-main"
                && ns.as_ref() != "workspace.default"
            {
                extra.push(p);
            }
        }
    }
    extra.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    out.extend(extra);
    out
}

fn skill_name_for_path(skill_path: &Path) -> String {
    metadata::parse_skill_metadata(skill_path)
        .ok()
        .filter(|m| !m.name.is_empty())
        .map(|m| m.name)
        .unwrap_or_else(|| {
            skill_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

fn enumerate_skills_in_root(root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if !root.is_dir() {
        return v;
    }
    let Ok(rd) = fs::read_dir(root) else {
        return v;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() && p.join("SKILL.md").exists() {
            v.push(p);
        }
    }
    v
}

fn skill_roots_in_order(project_root: &Path, openclaw_home: &Path) -> Vec<(PathBuf, String)> {
    let mut roots = Vec::new();
    for w in openclaw_workspace_candidates(project_root) {
        let label = w
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        let s = w.join("skills");
        if s.is_dir() {
            roots.push((s, format!("{label}/skills")));
        }
        let a = w.join(".agents").join("skills");
        if a.is_dir() {
            roots.push((a, format!("{label}/.agents/skills")));
        }
    }
    let oc_skills = openclaw_home.join("skills");
    if oc_skills.is_dir() {
        roots.push((oc_skills, "~/.openclaw/skills".to_string()));
    }
    if let Some(home) = dirs::home_dir() {
        for (rel, tag) in [
            (".clawdbot/skills", "~/.clawdbot/skills"),
            (".moltbot/skills", "~/.moltbot/skills"),
        ] {
            let p = home.join(rel);
            if p.is_dir() {
                roots.push((p, tag.to_string()));
            }
        }
        let ag = home.join(".agents/skills");
        if ag.is_dir() {
            roots.push((ag, "~/.agents/skills".to_string()));
        }
    }
    roots
}

/// Build ordered list: `(logical_name, source_path, source_tag)`; duplicate names keep first (highest priority).
pub fn collect_openclaw_import_candidates(
    project_root: &Path,
    openclaw_home: &Path,
) -> Vec<(String, PathBuf, String)> {
    let mut by_name: HashMap<String, (PathBuf, String)> = HashMap::new();
    for (root, tag) in skill_roots_in_order(project_root, openclaw_home) {
        for skill_path in enumerate_skills_in_root(&root) {
            let name = skill_name_for_path(&skill_path);
            if name.is_empty() {
                continue;
            }
            if by_name.contains_key(&name) {
                continue;
            }
            by_name.insert(name, (skill_path, tag.clone()));
        }
    }
    let mut v: Vec<_> = by_name
        .into_iter()
        .map(|(name, (path, tag))| (name, path, tag))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn unique_renamed(skills_path: &Path, name: &str) -> String {
    let mut candidate = format!("{name}-imported");
    let mut n = 2u32;
    while skills_path.join(&candidate).exists() {
        candidate = format!("{name}-imported-{n}");
        n += 1;
    }
    candidate
}

fn resolve_dest_name(
    skills_path: &Path,
    logical_name: &str,
    policy: SkillConflictPolicy,
) -> Option<String> {
    let dest = skills_path.join(logical_name);
    if !dest.exists() {
        return Some(logical_name.to_string());
    }
    match policy {
        SkillConflictPolicy::Skip => None,
        SkillConflictPolicy::Overwrite => Some(logical_name.to_string()),
        SkillConflictPolicy::Rename => Some(unique_renamed(skills_path, logical_name)),
    }
}

pub fn cmd_import_openclaw_skills(
    workspace: &str,
    openclaw_dir: Option<&str>,
    skills_dir: &str,
    skill_conflict: &str,
    dry_run: bool,
    force: bool,
    scan_offline: bool,
) -> Result<()> {
    let policy = SkillConflictPolicy::parse(skill_conflict)?;
    let project_root = PathBuf::from(workspace);
    let project_root = if project_root.is_absolute() {
        project_root
    } else {
        std::env::current_dir()
            .map_err(crate::Error::from)?
            .join(project_root)
    };
    let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);

    let openclaw_home = match openclaw_dir {
        Some(p) => {
            let pb = PathBuf::from(p);
            if pb.is_absolute() {
                pb
            } else {
                std::env::current_dir()
                    .map_err(crate::Error::from)?
                    .join(pb)
            }
        }
        None => dirs::home_dir()
            .ok_or_else(|| crate::Error::validation("Cannot resolve home directory"))?
            .join(".openclaw"),
    };
    let openclaw_home = fs::canonicalize(&openclaw_home).unwrap_or(openclaw_home);

    let skills_path = resolve_skills_dir(skills_dir);

    eprintln!("📂 Project root: {}", project_root.display());
    eprintln!("📂 OpenClaw home: {}", openclaw_home.display());
    eprintln!("📂 Destination skills dir: {}", skills_path.display());
    eprintln!();

    let candidates = collect_openclaw_import_candidates(&project_root, &openclaw_home);
    if candidates.is_empty() {
        eprintln!("No skills found under OpenClaw-style paths.");
        eprintln!("Checked workspace folders (workspace, workspace-main, …) for skills/ and .agents/skills,");
        eprintln!(
            "plus ~/.openclaw/skills, ~/.clawdbot/skills, ~/.moltbot/skills, ~/.agents/skills."
        );
        bail!("No skills to import");
    }

    eprintln!(
        "🔍 Found {} unique skill(s) (by SKILL.md name):",
        candidates.len()
    );
    for (name, path, tag) in &candidates {
        eprintln!("   • {}  ← {}  ({})", name, path.display(), tag);
    }
    eprintln!();

    let mut planned: Vec<(String, PathBuf, String, String)> = Vec::new();
    for (logical_name, src_path, tag) in &candidates {
        let Some(dest_name) = resolve_dest_name(&skills_path, logical_name, policy) else {
            eprintln!(
                "   ⏭ {}: destination exists (--skill-conflict skip)",
                logical_name
            );
            continue;
        };
        planned.push((
            dest_name.clone(),
            src_path.clone(),
            tag.clone(),
            logical_name.clone(),
        ));
    }

    if planned.is_empty() {
        eprintln!("Nothing to install (all skipped or empty after conflict resolution).");
        return Ok(());
    }

    if dry_run {
        eprintln!("Dry run — would install {} skill(s):", planned.len());
        for (dest_name, src, tag, logical) in &planned {
            if dest_name == logical {
                eprintln!("   • {} → {} [{}]", logical, dest_name, tag);
            } else {
                eprintln!("   • {} → {} (renamed) [{}]", logical, dest_name, tag);
            }
            eprintln!("     from {}", src.display());
        }
        return Ok(());
    }

    fs::create_dir_all(&skills_path).map_err(crate::Error::from)?;

    let install_candidates: Vec<(String, PathBuf)> = planned
        .iter()
        .map(|(dest_name, src, _, _)| (dest_name.clone(), src.clone()))
        .collect();

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
    let blocked: HashSet<&str> = malicious.iter().map(String::as_str).collect();
    let skipped_suspicious: HashSet<&str> = if force {
        HashSet::new()
    } else {
        suspicious.iter().map(String::as_str).collect()
    };

    let source_prefix = "openclaw-import";
    let mut installed: Vec<String> = Vec::new();
    for (dest_name, src_path, tag, _logical) in &planned {
        if blocked.contains(dest_name.as_str()) {
            continue;
        }
        if skipped_suspicious.contains(dest_name.as_str()) {
            continue;
        }
        let dest = skills_path.join(dest_name);
        copy_skill(src_path, &dest)?;
        let admission = risk_by_name.get(dest_name).copied();
        let source = format!("{source_prefix}:{tag}");
        let _entry = manifest::upsert_installed_skill_with_admission(
            &skills_path,
            &dest,
            &source,
            admission,
        )?;
        installed.push(dest_name.clone());
        eprintln!("   ✓ {}: installed to {}", dest_name, dest.display());
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
        "🎉 Imported {} skill(s) from OpenClaw-style locations",
        installed.len()
    );
    for name in &installed {
        eprintln!("  • {}", name);
    }
    eprintln!("{}", "=".repeat(50));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_workspace_before_workspace_main() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let ws = project.join("workspace");
        fs::create_dir_all(ws.join("skills/a")).unwrap();
        fs::write(ws.join("skills/a/SKILL.md"), "---\nname: shared\n---\n").unwrap();
        let ws_main = project.join("workspace-main");
        fs::create_dir_all(ws_main.join("skills/b")).unwrap();
        fs::write(
            ws_main.join("skills/b/SKILL.md"),
            "---\nname: shared\n---\nother\n",
        )
        .unwrap();

        let openclaw_home = project.join(".openclaw");
        fs::create_dir_all(&openclaw_home).unwrap();

        let c = collect_openclaw_import_candidates(project, &openclaw_home);
        let shared = c.iter().find(|(n, _, _)| n == "shared").unwrap();
        assert!(
            shared.1.starts_with(ws.join("skills")),
            "expected workspace/ copy, got {:?}",
            shared.1
        );
    }

    #[test]
    fn rename_policy_generates_imported_suffix() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path();
        fs::create_dir_all(skills.join("x")).unwrap();
        assert_eq!(unique_renamed(skills, "x").as_str(), "x-imported");
        fs::create_dir_all(skills.join("x-imported")).unwrap();
        assert_eq!(unique_renamed(skills, "x").as_str(), "x-imported-2");
    }
}
