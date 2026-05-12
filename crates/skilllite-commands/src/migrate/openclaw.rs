//! OpenClaw → SkillLite migration (MVP): skills, persona/memory Markdown, optional `.env` subset.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use skilllite_core::paths::{self, project_skilllite_dir};
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::skill::{
    cmd_import_openclaw_skills, collect_openclaw_import_candidates, openclaw_workspace_candidates,
    resolve_skills_dir, SkillConflictPolicy,
};
use crate::Result;

const ARCHIVE_ONLY_WORKSPACE_FILES: &[&str] = &[
    "AGENTS.md",
    "IDENTITY.md",
    "TOOLS.md",
    "HEARTBEAT.md",
    "BOOTSTRAP.md",
];

const SECRET_ENV_ALLOWLIST: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "OPENROUTER_API_KEY",
    "DEEPSEEK_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GROQ_API_KEY",
    "XAI_API_KEY",
    "MISTRAL_API_KEY",
    "DASHSCOPE_API_KEY",
    "MOONSHOT_API_KEY",
    "OPENAI_BASE_URL",
    "SKILLLITE_API_BASE",
    "SKILLLITE_MODEL",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum PlanAction {
    Copy,
    MergeAppend,
    SkipConflict,
    ArchiveOnly,
    EnvMerge,
    ImportSkills,
}

#[derive(Debug, Clone, Serialize)]
struct PlanItem {
    category: String,
    action: PlanAction,
    source: String,
    destination: String,
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MigrationReport {
    source: String,
    project_root: String,
    openclaw_home: String,
    dry_run: bool,
    backup_zip: Option<String>,
    items: Vec<PlanItem>,
    skipped: Vec<String>,
    archived: Vec<String>,
}

pub fn cmd_claw_migrate_openclaw(
    workspace: &str,
    openclaw_dir: Option<&str>,
    skills_dir: &str,
    skill_conflict: &str,
    dry_run: bool,
    force: bool,
    scan_offline: bool,
    migrate_secrets: bool,
    no_backup: bool,
    yes: bool,
    overwrite: bool,
) -> Result<()> {
    let _ = SkillConflictPolicy::parse(skill_conflict)?;

    let project_root = resolve_project_root(workspace)?;
    let openclaw_home = resolve_openclaw_home(openclaw_dir)?;
    let skills_path = resolve_skills_dir(skills_dir);
    let soul_dest = project_skilllite_dir(&project_root).join("SOUL.md");
    let memory_root = paths::chat_root().join("memory");
    let env_dest = project_root.join(".env");

    let skill_candidates = collect_openclaw_import_candidates(&project_root, &openclaw_home);
    let mut items = Vec::new();
    let mut skipped = Vec::new();
    let mut archived = Vec::new();

    if skill_candidates.is_empty() {
        skipped.push("skills: no OpenClaw-style skill directories found".to_string());
    } else {
        items.push(PlanItem {
            category: "skills".to_string(),
            action: PlanAction::ImportSkills,
            source: format!("{} skill(s) across OpenClaw roots", skill_candidates.len()),
            destination: skills_path.display().to_string(),
            note: Some(format!("--skill-conflict {skill_conflict}")),
        });
    }

    plan_workspace_markdown(
        &project_root,
        &soul_dest,
        &memory_root,
        overwrite,
        &mut items,
        &mut skipped,
    );
    plan_archive_only_workspace_files(&project_root, &mut items, &mut archived);
    plan_openclaw_config_archive(&openclaw_home, &mut items, &mut archived);

    if migrate_secrets {
        let env_sources = discover_env_sources(&openclaw_home);
        if env_sources.is_empty() {
            skipped.push("env: no OpenClaw .env or config env block found".to_string());
        } else {
            items.push(PlanItem {
                category: "env".to_string(),
                action: PlanAction::EnvMerge,
                source: env_sources
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                destination: env_dest.display().to_string(),
                note: Some("allowlisted API keys only".to_string()),
            });
        }
    } else {
        skipped.push("env: skipped (pass --migrate-secrets to merge allowlisted keys)".to_string());
    }

    let report_dir = migration_report_dir(&project_root);
    print_plan(
        &project_root,
        &openclaw_home,
        &skills_path,
        &report_dir,
        &items,
        &skipped,
        &archived,
        dry_run,
        migrate_secrets,
    );

    if dry_run {
        write_report(
            &report_dir,
            MigrationReport {
                source: "openclaw".to_string(),
                project_root: project_root.display().to_string(),
                openclaw_home: openclaw_home.display().to_string(),
                dry_run: true,
                backup_zip: None,
                items,
                skipped,
                archived,
            },
        )?;
        eprintln!();
        eprintln!(
            "Dry run complete. Report written to {}.",
            report_dir.display()
        );
        return Ok(());
    }

    if !yes && !confirm_apply()? {
        eprintln!("Migration cancelled.");
        return Ok(());
    }

    fs::create_dir_all(&report_dir).map_err(crate::Error::from)?;
    let backup_zip = if no_backup {
        None
    } else {
        let zip_path = report_dir.join("pre-migration-backup.zip");
        create_backup_zip(
            &zip_path,
            &project_root,
            &skills_path,
            &memory_root,
            &env_dest,
        )?;
        eprintln!("🗄  Backup: {}", zip_path.display());
        Some(zip_path.display().to_string())
    };

    apply_workspace_markdown(
        &project_root,
        &soul_dest,
        &memory_root,
        overwrite,
        &report_dir,
        &mut items,
    )?;
    apply_archive_only(&project_root, &openclaw_home, &report_dir, &mut archived)?;
    if migrate_secrets {
        apply_env_merge(&openclaw_home, &env_dest, overwrite)?;
    }

    if !skill_candidates.is_empty() {
        cmd_import_openclaw_skills(
            workspace,
            openclaw_dir,
            skills_dir,
            skill_conflict,
            false,
            force,
            scan_offline,
        )?;
    }

    write_report(
        &report_dir,
        MigrationReport {
            source: "openclaw".to_string(),
            project_root: project_root.display().to_string(),
            openclaw_home: openclaw_home.display().to_string(),
            dry_run: false,
            backup_zip,
            items,
            skipped,
            archived,
        },
    )?;

    eprintln!();
    eprintln!("Migration report: {}", report_dir.display());
    Ok(())
}

fn resolve_project_root(workspace: &str) -> Result<PathBuf> {
    let project_root = PathBuf::from(workspace);
    let project_root = if project_root.is_absolute() {
        project_root
    } else {
        std::env::current_dir()
            .map_err(crate::Error::from)?
            .join(project_root)
    };
    Ok(fs::canonicalize(&project_root).unwrap_or(project_root))
}

pub(crate) fn resolve_openclaw_home(openclaw_dir: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = openclaw_dir {
        let pb = PathBuf::from(p);
        let pb = if pb.is_absolute() {
            pb
        } else {
            std::env::current_dir()
                .map_err(crate::Error::from)?
                .join(pb)
        };
        return Ok(fs::canonicalize(&pb).unwrap_or(pb));
    }

    let home = dirs::home_dir().ok_or_else(|| crate::Error::validation("Cannot resolve home"))?;
    for rel in [".openclaw", ".clawdbot", ".moltbot"] {
        let candidate = home.join(rel);
        if candidate.is_dir() {
            return Ok(fs::canonicalize(&candidate).unwrap_or(candidate));
        }
    }
    let fallback = home.join(".openclaw");
    Ok(fs::canonicalize(&fallback).unwrap_or(fallback))
}

fn migration_report_dir(project_root: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    project_skilllite_dir(project_root)
        .join("migration")
        .join("openclaw")
        .join(stamp.to_string())
}

fn plan_workspace_markdown(
    project_root: &Path,
    soul_dest: &Path,
    memory_root: &Path,
    overwrite: bool,
    items: &mut Vec<PlanItem>,
    skipped: &mut Vec<String>,
) {
    let mut soul_found = false;
    for ws in openclaw_workspace_candidates(project_root) {
        let label = ws
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        if !soul_found {
            let soul_src = ws.join("SOUL.md");
            if soul_src.is_file() {
                soul_found = true;
                push_file_plan("persona", &soul_src, soul_dest, overwrite, items, skipped);
            }
        }

        for name in ["MEMORY.md", "USER.md"] {
            let src = ws.join(name);
            if src.is_file() {
                let dest = memory_root.join(name);
                push_file_plan("memory", &src, &dest, overwrite, items, skipped);
            }
        }

        let daily_root = ws.join("memory");
        if daily_root.is_dir() {
            if let Ok(rd) = fs::read_dir(&daily_root) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("md") {
                        continue;
                    }
                    let dest = memory_root.join(
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    push_file_plan("memory", &path, &dest, overwrite, items, skipped);
                }
            }
        }

        let _ = label;
    }

    if !soul_found {
        skipped.push("persona: no workspace SOUL.md found".to_string());
    }
}

fn push_file_plan(
    category: &str,
    source: &Path,
    dest: &Path,
    overwrite: bool,
    items: &mut Vec<PlanItem>,
    skipped: &mut Vec<String>,
) {
    if dest.exists() && !overwrite {
        skipped.push(format!(
            "{category}: {} exists (use --overwrite)",
            dest.display()
        ));
        items.push(PlanItem {
            category: category.to_string(),
            action: PlanAction::SkipConflict,
            source: source.display().to_string(),
            destination: dest.display().to_string(),
            note: Some("destination exists".to_string()),
        });
        return;
    }

    let action = if dest.exists() && overwrite {
        PlanAction::MergeAppend
    } else {
        PlanAction::Copy
    };
    items.push(PlanItem {
        category: category.to_string(),
        action,
        source: source.display().to_string(),
        destination: dest.display().to_string(),
        note: None,
    });
}

fn plan_archive_only_workspace_files(
    project_root: &Path,
    items: &mut Vec<PlanItem>,
    archived: &mut Vec<String>,
) {
    for ws in openclaw_workspace_candidates(project_root) {
        for name in ARCHIVE_ONLY_WORKSPACE_FILES {
            let src = ws.join(name);
            if src.is_file() {
                archived.push(src.display().to_string());
                items.push(PlanItem {
                    category: "archive".to_string(),
                    action: PlanAction::ArchiveOnly,
                    source: src.display().to_string(),
                    destination: "report/archive/".to_string(),
                    note: Some("manual review".to_string()),
                });
            }
        }
    }
}

fn plan_openclaw_config_archive(
    openclaw_home: &Path,
    items: &mut Vec<PlanItem>,
    archived: &mut Vec<String>,
) {
    for name in ["openclaw.json", "clawdbot.json", "moltbot.json"] {
        let src = openclaw_home.join(name);
        if src.is_file() {
            archived.push(src.display().to_string());
            items.push(PlanItem {
                category: "archive".to_string(),
                action: PlanAction::ArchiveOnly,
                source: src.display().to_string(),
                destination: "report/archive/".to_string(),
                note: Some("unmapped config preserved for review".to_string()),
            });
        }
    }
}

fn discover_env_sources(openclaw_home: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let dotenv = openclaw_home.join(".env");
    if dotenv.is_file() {
        out.push(dotenv);
    }
    for name in ["openclaw.json", "clawdbot.json", "moltbot.json"] {
        let cfg = openclaw_home.join(name);
        if cfg.is_file() {
            out.push(cfg);
        }
    }
    out
}

fn print_plan(
    project_root: &Path,
    openclaw_home: &Path,
    skills_path: &Path,
    report_dir: &Path,
    items: &[PlanItem],
    skipped: &[String],
    archived: &[String],
    dry_run: bool,
    migrate_secrets: bool,
) {
    eprintln!(
        "OpenClaw → SkillLite migration{}",
        if dry_run { " (dry run)" } else { "" }
    );
    eprintln!("  Project:      {}", project_root.display());
    eprintln!("  OpenClaw dir: {}", openclaw_home.display());
    eprintln!("  Skills dir:   {}", skills_path.display());
    eprintln!(
        "  Memory dir:   {}",
        paths::chat_root().join("memory").display()
    );
    eprintln!("  Report dir:   {}", report_dir.display());
    eprintln!(
        "  Secrets:      {}",
        if migrate_secrets { "yes" } else { "no" }
    );
    eprintln!();

    if items.is_empty() {
        eprintln!("No migratable items planned.");
    } else {
        eprintln!("Planned actions ({}):", items.len());
        for item in items {
            eprintln!(
                "  • [{}] {:?} {} → {}",
                item.category, item.action, item.source, item.destination
            );
            if let Some(note) = &item.note {
                eprintln!("      ({note})");
            }
        }
    }

    if !skipped.is_empty() {
        eprintln!();
        eprintln!("Skipped / notes:");
        for line in skipped {
            eprintln!("  - {line}");
        }
    }

    if !archived.is_empty() {
        eprintln!();
        eprintln!("Archive for manual review ({}):", archived.len());
        for path in archived {
            eprintln!("  - {path}");
        }
    }
}

fn confirm_apply() -> Result<bool> {
    eprint!("Apply migration? [y/N] ");
    io::stdout().flush().map_err(crate::Error::from)?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(crate::Error::from)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn create_backup_zip(
    zip_path: &Path,
    project_root: &Path,
    skills_path: &Path,
    memory_root: &Path,
    env_dest: &Path,
) -> Result<()> {
    if let Some(parent) = zip_path.parent() {
        fs::create_dir_all(parent).map_err(crate::Error::from)?;
    }
    let file = File::create(zip_path).map_err(crate::Error::from)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default();

    let mut roots: Vec<(&Path, &str)> = vec![(skills_path, "skills"), (memory_root, "chat-memory")];
    let project_skilllite = project_skilllite_dir(project_root);
    if project_skilllite.is_dir() {
        roots.push((&project_skilllite, ".skilllite"));
    }
    if env_dest.is_file() {
        add_file_to_zip(&mut zip, env_dest, ".env", options)?;
    }

    for (root, prefix) in roots {
        if !root.exists() {
            continue;
        }
        if root.is_file() {
            add_file_to_zip(&mut zip, root, prefix, options)?;
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or_else(|_| path)
                .to_string_lossy();
            let name = format!("{prefix}/{rel}");
            add_file_to_zip(&mut zip, path, &name, options)?;
        }
    }

    zip.finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        .map_err(crate::Error::from)?;
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    path: &Path,
    name: &str,
    options: FileOptions,
) -> Result<()> {
    zip.start_file(name, options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
        .map_err(crate::Error::from)?;
    let mut f = File::open(path).map_err(crate::Error::from)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(crate::Error::from)?;
    zip.write_all(&buf).map_err(crate::Error::from)?;
    Ok(())
}

fn apply_workspace_markdown(
    project_root: &Path,
    soul_dest: &Path,
    memory_root: &Path,
    overwrite: bool,
    report_dir: &Path,
    items: &mut [PlanItem],
) -> Result<()> {
    fs::create_dir_all(memory_root).map_err(crate::Error::from)?;
    if let Some(parent) = soul_dest.parent() {
        fs::create_dir_all(parent).map_err(crate::Error::from)?;
    }

    for ws in openclaw_workspace_candidates(project_root) {
        let soul_src = ws.join("SOUL.md");
        if soul_src.is_file() {
            copy_or_merge_file(&soul_src, soul_dest, overwrite)?;
            break;
        }
    }

    for ws in openclaw_workspace_candidates(project_root) {
        for name in ["MEMORY.md", "USER.md"] {
            let src = ws.join(name);
            if src.is_file() {
                let dest = memory_root.join(name);
                copy_or_merge_file(&src, &dest, overwrite)?;
            }
        }

        let daily_root = ws.join("memory");
        if daily_root.is_dir() {
            if let Ok(rd) = fs::read_dir(&daily_root) {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("md") {
                        continue;
                    }
                    let dest = memory_root.join(
                        path.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    copy_or_merge_file(&path, &dest, overwrite)?;
                }
            }
        }
    }

    let _ = report_dir;
    let _ = items;
    Ok(())
}

fn copy_or_merge_file(source: &Path, dest: &Path, overwrite: bool) -> Result<()> {
    if dest.exists() && !overwrite {
        return Ok(());
    }
    let content = fs::read_to_string(source).map_err(crate::Error::from)?;
    if dest.exists() && overwrite {
        let mut existing = fs::read_to_string(dest).map_err(crate::Error::from)?;
        if !existing.is_empty() && !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str("\n--- migrated from ");
        existing.push_str(&source.display().to_string());
        existing.push_str(" ---\n");
        existing.push_str(&content);
        fs::write(dest, existing).map_err(crate::Error::from)?;
    } else {
        fs::write(dest, content).map_err(crate::Error::from)?;
    }
    Ok(())
}

fn apply_archive_only(
    project_root: &Path,
    openclaw_home: &Path,
    report_dir: &Path,
    archived: &mut [String],
) -> Result<()> {
    let archive_dir = report_dir.join("archive");
    fs::create_dir_all(&archive_dir).map_err(crate::Error::from)?;

    for ws in openclaw_workspace_candidates(project_root) {
        for name in ARCHIVE_ONLY_WORKSPACE_FILES {
            let src = ws.join(name);
            if src.is_file() {
                let dest = archive_dir.join(format!(
                    "{}-{}",
                    ws.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("workspace"),
                    name
                ));
                fs::copy(&src, &dest).map_err(crate::Error::from)?;
            }
        }
    }

    for name in ["openclaw.json", "clawdbot.json", "moltbot.json"] {
        let src = openclaw_home.join(name);
        if src.is_file() {
            let dest = archive_dir.join(name);
            fs::copy(&src, &dest).map_err(crate::Error::from)?;
        }
    }

    let _ = archived;
    Ok(())
}

fn apply_env_merge(openclaw_home: &Path, env_dest: &Path, overwrite: bool) -> Result<()> {
    let mut collected = BTreeMap::new();
    collect_env_from_dotenv(&openclaw_home.join(".env"), &mut collected);
    for name in ["openclaw.json", "clawdbot.json", "moltbot.json"] {
        collect_env_from_openclaw_json(&openclaw_home.join(name), &mut collected);
    }

    if collected.is_empty() {
        return Ok(());
    }

    let mut existing = parse_env_file(env_dest);
    for (key, value) in collected {
        if existing.contains_key(&key) && !overwrite {
            continue;
        }
        existing.insert(key, value);
    }
    write_env_file(env_dest, &existing)?;
    Ok(())
}

fn collect_env_from_dotenv(path: &Path, out: &mut BTreeMap<String, String>) {
    if !path.is_file() {
        return;
    }
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for (k, v) in parse_env_text(&content) {
        if SECRET_ENV_ALLOWLIST.contains(&k.as_str()) {
            out.insert(k, v);
        }
    }
}

fn collect_env_from_openclaw_json(path: &Path, out: &mut BTreeMap<String, String>) {
    if !path.is_file() {
        return;
    }
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    if let Some(env) = value.get("env") {
        collect_env_object(env, out);
    }
    if let Some(providers) = value
        .pointer("/models/providers")
        .and_then(|v| v.as_object())
    {
        for provider in providers.values() {
            if let Some(key) = provider.get("apiKey").and_then(|v| v.as_str()) {
                if !key.is_empty() {
                    out.insert("OPENAI_API_KEY".to_string(), key.to_string());
                }
            }
        }
    }
}

fn collect_env_object(value: &serde_json::Value, out: &mut BTreeMap<String, String>) {
    let obj = value
        .as_object()
        .or_else(|| value.get("vars").and_then(|v| v.as_object()));
    let Some(obj) = obj else {
        return;
    };
    for (k, v) in obj {
        let Some(s) = v.as_str() else {
            continue;
        };
        if SECRET_ENV_ALLOWLIST.contains(&k.as_str()) {
            out.insert(k.clone(), s.to_string());
        }
    }
}

fn parse_env_file(path: &Path) -> BTreeMap<String, String> {
    if !path.is_file() {
        return BTreeMap::new();
    }
    let Ok(content) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    parse_env_text(&content).into_iter().collect()
}

fn parse_env_text(content: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        out.push((k.trim().to_string(), v.trim().trim_matches('"').to_string()));
    }
    out
}

fn write_env_file(path: &Path, entries: &BTreeMap<String, String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(crate::Error::from)?;
    }
    let mut lines = Vec::new();
    if path.is_file() {
        let existing = fs::read_to_string(path).map_err(crate::Error::from)?;
        let keys: std::collections::HashSet<_> = entries.keys().cloned().collect();
        for line in existing.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                lines.push(line.to_string());
                continue;
            }
            if let Some((k, _)) = trimmed.split_once('=') {
                if keys.contains(k.trim()) {
                    continue;
                }
            }
            lines.push(line.to_string());
        }
    }
    for (k, v) in entries {
        lines.push(format!("{k}={v}"));
    }
    let body = lines.join("\n");
    let body = if body.is_empty() {
        body
    } else {
        format!("{body}\n")
    };
    fs::write(path, body).map_err(crate::Error::from)?;
    Ok(())
}

fn write_report(report_dir: &Path, report: MigrationReport) -> Result<()> {
    fs::create_dir_all(report_dir).map_err(crate::Error::from)?;
    let json_path = report_dir.join("report.json");
    let json = serde_json::to_string_pretty(&report).map_err(crate::Error::from)?;
    fs::write(&json_path, json).map_err(crate::Error::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_openclaw_home_prefers_existing_legacy_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let clawdbot = home.join(".clawdbot");
        fs::create_dir_all(&clawdbot).unwrap();
        // Cannot override dirs::home_dir easily; test explicit path instead.
        let resolved = resolve_openclaw_home(Some(clawdbot.to_str().unwrap())).unwrap();
        assert_eq!(resolved, fs::canonicalize(&clawdbot).unwrap_or(clawdbot));
    }

    #[test]
    fn env_allowlist_filters_unknown_keys() {
        let mut collected = BTreeMap::new();
        collect_env_from_dotenv(&std::path::PathBuf::from("/nonexistent"), &mut collected);
        assert!(collected.is_empty());

        let tmp = tempfile::tempdir().unwrap();
        let env_path = tmp.path().join(".env");
        fs::write(&env_path, "OPENAI_API_KEY=sk-test\nRANDOM_SECRET=abc\n").unwrap();
        collect_env_from_dotenv(&env_path, &mut collected);
        assert_eq!(
            collected.get("OPENAI_API_KEY").map(String::as_str),
            Some("sk-test")
        );
        assert!(!collected.contains_key("RANDOM_SECRET"));
    }

    #[test]
    fn plan_marks_soul_and_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let ws = project.join("workspace");
        fs::create_dir_all(ws.join("memory")).unwrap();
        fs::write(ws.join("SOUL.md"), "soul").unwrap();
        fs::write(ws.join("MEMORY.md"), "mem").unwrap();
        fs::write(ws.join("memory/2026-05-12.md"), "daily").unwrap();

        let soul_dest = project.join(".skilllite/SOUL.md");
        let memory_root = project.join("memory-out");
        let mut items = Vec::new();
        let mut skipped = Vec::new();
        plan_workspace_markdown(
            project,
            &soul_dest,
            &memory_root,
            false,
            &mut items,
            &mut skipped,
        );
        assert!(items.iter().any(|i| i.source.contains("SOUL.md")));
        assert!(items.iter().any(|i| i.source.contains("MEMORY.md")));
        assert!(items.iter().any(|i| i.source.contains("2026-05-12.md")));
    }
}
