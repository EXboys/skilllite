//! Skill management commands: add, remove, list, show.
//!
//! Migrated from Python `skilllite-sdk/skilllite/cli/add.py` and `repo.py`.
//! Depends ONLY on skill/ and env/ layers (Layer 1-2), NOT on agent/ (Layer 3).

use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::skill::metadata;

// ─── Source Parsing ─────────────────────────────────────────────────────────

/// Parsed result of a source string.
#[derive(Debug)]
struct ParsedSource {
    /// Source type: "github", "gitlab", "git", "local"
    source_type: String,
    /// Clone URL or local path
    url: String,
    /// Git ref (branch/tag)
    git_ref: Option<String>,
    /// Subdirectory within the repo
    subpath: Option<String>,
    /// Filter to a specific skill by name
    skill_filter: Option<String>,
}

fn is_local_path(source: &str) -> bool {
    Path::new(source).is_absolute()
        || source.starts_with("./")
        || source.starts_with("../")
        || source == "."
        || source == ".."
}

fn parse_source(source: &str) -> ParsedSource {
    // Local path
    if is_local_path(source) {
        let abs = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(source);
        return ParsedSource {
            source_type: "local".into(),
            url: abs.to_string_lossy().into(),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    // GitHub tree URL with path: https://github.com/owner/repo/tree/branch/path
    let re_tree_path =
        Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)").unwrap();
    if let Some(cap) = re_tree_path.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: Some(cap[4].to_string()),
            skill_filter: None,
        };
    }

    // GitHub tree URL branch only: https://github.com/owner/repo/tree/branch
    let re_tree_branch =
        Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)$").unwrap();
    if let Some(cap) = re_tree_branch.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: None,
            skill_filter: None,
        };
    }

    // GitHub URL: https://github.com/owner/repo
    let re_github = Regex::new(r"github\.com/([^/]+)/([^/]+?)(?:\.git)?/*$").unwrap();
    if let Some(cap) = re_github.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    // GitLab URL: https://gitlab.com/owner/repo
    let re_gitlab = Regex::new(r"gitlab\.com/(.+?)(?:\.git)?/?$").unwrap();
    if let Some(cap) = re_gitlab.captures(source) {
        let repo_path = &cap[1];
        if repo_path.contains('/') {
            return ParsedSource {
                source_type: "gitlab".into(),
                url: format!("https://gitlab.com/{}.git", repo_path),
                git_ref: None,
                subpath: None,
                skill_filter: None,
            };
        }
    }

    // GitHub shorthand with @ filter: owner/repo@skill-name
    let re_at_filter = Regex::new(r"^([^/]+)/([^/@]+)@(.+)$").unwrap();
    if let Some(cap) = re_at_filter.captures(source) {
        if !source.contains(':') {
            return ParsedSource {
                source_type: "github".into(),
                url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
                git_ref: None,
                subpath: None,
                skill_filter: Some(cap[3].to_string()),
            };
        }
    }

    // GitHub shorthand: owner/repo or owner/repo/path/to/skill
    let re_shorthand = Regex::new(r"^([^/]+)/([^/]+)(?:/(.+))?$").unwrap();
    if let Some(cap) = re_shorthand.captures(source) {
        if !source.contains(':') && !source.starts_with('.') {
            return ParsedSource {
                source_type: "github".into(),
                url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
                git_ref: None,
                subpath: cap.get(3).map(|m| m.as_str().to_string()),
                skill_filter: None,
            };
        }
    }

    // Fallback: treat as direct git URL
    ParsedSource {
        source_type: "git".into(),
        url: source.to_string(),
        git_ref: None,
        subpath: None,
        skill_filter: None,
    }
}

// ─── Git Clone ──────────────────────────────────────────────────────────────

fn clone_repo(url: &str, git_ref: Option<&str>) -> Result<PathBuf> {
    let temp_dir = tempfile::tempdir()
        .context("Failed to create temp directory")?;
    #[allow(deprecated)]
    let temp_path = temp_dir.into_path();

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--depth", "1"]);
    if let Some(r) = git_ref {
        cmd.args(["--branch", r]);
    }
    cmd.arg(url).arg(&temp_path);

    let output = cmd
        .output()
        .context("Failed to execute git clone. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&temp_path);
        if stderr.contains("Authentication failed") || stderr.contains("Permission denied") {
            anyhow::bail!(
                "Authentication failed for {}.\n  For private repos, ensure you have access.\n  For SSH: ssh -T git@github.com\n  For HTTPS: gh auth login",
                url
            );
        }
        anyhow::bail!("Failed to clone {}: {}", url, stderr.trim());
    }

    Ok(temp_path)
}

// ─── Skill Discovery ────────────────────────────────────────────────────────

const SKILL_SEARCH_DIRS: &[&str] = &["skills", ".skills", ".agents/skills", ".claude/skills", "."];

fn discover_skills(
    repo_dir: &Path,
    subpath: Option<&str>,
    skill_filter: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(sp) = subpath {
        let target = repo_dir.join(sp);
        if target.is_dir() && target.join("SKILL.md").exists() {
            return vec![target];
        }
        if target.is_dir() {
            if let Ok(entries) = fs::read_dir(&target) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").exists() {
                        candidates.push(p);
                    }
                }
                if !candidates.is_empty() {
                    candidates.sort();
                    return candidates;
                }
            }
        }
        let skill_name = sp.split('/').last().unwrap_or(sp);
        for search_dir in SKILL_SEARCH_DIRS {
            if *search_dir == "." {
                continue;
            }
            let candidate = repo_dir.join(search_dir).join(skill_name);
            if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                candidates.push(candidate);
            }
        }
        if candidates.is_empty() && sp.contains('/') {
            for search_dir in SKILL_SEARCH_DIRS {
                if *search_dir == "." {
                    continue;
                }
                let candidate = repo_dir.join(search_dir).join(sp);
                if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                    candidates.push(candidate);
                }
            }
        }
        if candidates.is_empty() {
            find_skill_by_name_recursive(repo_dir, skill_name, &mut candidates);
        }
        return candidates;
    }

    let mut seen = std::collections::HashSet::new();
    for search_dir in SKILL_SEARCH_DIRS {
        let search_path = repo_dir.join(search_dir);
        if !search_path.is_dir() {
            continue;
        }
        if *search_dir == "." {
            if let Ok(entries) = fs::read_dir(&search_path) {
                let mut children: Vec<_> = entries.flatten().collect();
                children.sort_by_key(|e| e.file_name());
                for entry in children {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").exists() {
                        if let Ok(real) = p.canonicalize() {
                            if seen.insert(real) {
                                candidates.push(p);
                            }
                        }
                    }
                }
            }
        } else {
            if search_path.join("SKILL.md").exists() {
                if let Ok(real) = search_path.canonicalize() {
                    if seen.insert(real) {
                        candidates.push(search_path.clone());
                    }
                }
            }
            if let Ok(entries) = fs::read_dir(&search_path) {
                let mut children: Vec<_> = entries.flatten().collect();
                children.sort_by_key(|e| e.file_name());
                for entry in children {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").exists() {
                        if let Ok(real) = p.canonicalize() {
                            if seen.insert(real) {
                                candidates.push(p);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(filter) = skill_filter {
        candidates.retain(|c| {
            c.file_name()
                .map(|n| n.to_string_lossy() == filter)
                .unwrap_or(false)
        });
    }

    candidates
}

fn find_skill_by_name_recursive(dir: &Path, name: &str, results: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let dir_name = p.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.starts_with('.') || dir_name == "node_modules" || dir_name == "__pycache__" {
                continue;
            }
            if dir_name == name && p.join("SKILL.md").exists() {
                results.push(p.clone());
            }
            find_skill_by_name_recursive(&p, name, results);
        }
    }
}

// ─── Skill Copy ─────────────────────────────────────────────────────────────

fn copy_skill(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest)
            .with_context(|| format!("Failed to remove existing skill: {}", dest.display()))?;
    }
    copy_dir_filtered(src, dest)?;
    Ok(())
}

fn copy_dir_filtered(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory: {}", dest.display()))?;
    for entry in fs::read_dir(src)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == ".git"
            || name_str == "__pycache__"
            || name_str == ".DS_Store"
            || name_str.ends_with(".pyc")
        {
            continue;
        }
        let src_path = entry.path();
        let dest_path = dest.join(&name);
        if src_path.is_dir() {
            copy_dir_filtered(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .with_context(|| format!("Failed to copy: {}", src_path.display()))?;
        }
    }
    Ok(())
}

// ─── Dependency Installation ────────────────────────────────────────────────

fn install_skill_deps(skills_dir: &Path, installed: &[String]) -> Vec<String> {
    let mut messages = Vec::new();
    for name in installed {
        let skill_path = skills_dir.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }
        match metadata::parse_skill_metadata(&skill_path) {
            Ok(meta) => {
                let cache_dir: Option<&str> = None;
                match crate::env::builder::ensure_environment(&skill_path, &meta, cache_dir) {
                    Ok(_) => {
                        let lang = metadata::detect_language(&skill_path, &meta);
                        messages.push(format!(
                            "   ✓ {} [{}]: dependencies installed",
                            name, lang
                        ));
                    }
                    Err(e) => {
                        messages.push(format!("   ✗ {}: dependency error: {}", name, e));
                    }
                }
            }
            Err(e) => {
                messages.push(format!("   ✗ {}: parse error: {}", name, e));
            }
        }
    }
    messages
}

// ═══════════════════════════════════════════════════════════════════════════
// Public command handlers
// ═══════════════════════════════════════════════════════════════════════════

/// `skillbox skill add <source>`
pub fn cmd_add(source: &str, skills_dir: &str, force: bool, list_only: bool) -> Result<()> {
    let skills_path = resolve_skills_dir(skills_dir);
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
                anyhow::bail!("Local path does not exist: {}", parsed.url);
            }
            eprintln!("📁 Using local path: {}", parsed.url);
            p
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
            anyhow::bail!("No skills found");
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
        fs::create_dir_all(&skills_path)
            .context("Failed to create skills directory")?;

        let mut installed: Vec<String> = Vec::new();
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

            copy_skill(skill_path, &dest)?;
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

/// `skillbox skill remove <name>`
pub fn cmd_remove(skill_name: &str, skills_dir: &str, force: bool) -> Result<()> {
    let skills_path = resolve_skills_dir(skills_dir);

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
    eprintln!("✓ Removed skill '{}'", dir_name);
    Ok(())
}

/// `skillbox skill list`
pub fn cmd_list(skills_dir: &str, json_output: bool) -> Result<()> {
    let skills_path = resolve_skills_dir(skills_dir);

    if !skills_path.exists() {
        if json_output {
            println!("[]");
        } else {
            eprintln!("No skills directory found. Run `skillbox skill add` first.");
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
            let info = skill_to_json(skill_path);
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

/// `skillbox skill show <name>`
pub fn cmd_show(skill_name: &str, skills_dir: &str, json_output: bool) -> Result<()> {
    let skills_path = resolve_skills_dir(skills_dir);
    let skill_path = find_skill(&skills_path, skill_name)?;
    let meta = metadata::parse_skill_metadata(&skill_path)?;
    let lang = metadata::detect_language(&skill_path, &meta);

    if json_output {
        let info = skill_to_json(&skill_path);
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    eprintln!("📦 Skill: {}", meta.name);
    eprintln!("   Path: {}", skill_path.display());
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
        if let Ok(entries) = fs::read_dir(&scripts_dir) {
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
        if let Ok(entries) = fs::read_dir(&refs_dir) {
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

// ─── Helpers ────────────────────────────────────────────────────────────────

fn resolve_skills_dir(skills_dir: &str) -> PathBuf {
    let p = PathBuf::from(skills_dir);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

fn find_skill(skills_path: &Path, skill_name: &str) -> Result<PathBuf> {
    if !skills_path.exists() {
        anyhow::bail!("Skills directory not found: {}", skills_path.display());
    }

    let direct = skills_path.join(skill_name);
    if direct.is_dir() && direct.join("SKILL.md").exists() {
        return Ok(direct);
    }

    if let Ok(entries) = fs::read_dir(skills_path) {
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

fn skill_to_json(skill_path: &Path) -> serde_json::Value {
    match metadata::parse_skill_metadata(skill_path) {
        Ok(meta) => {
            let lang = metadata::detect_language(skill_path, &meta);
            serde_json::json!({
                "name": meta.name,
                "description": meta.description,
                "language": lang,
                "entry_point": if meta.entry_point.is_empty() { None } else { Some(&meta.entry_point) },
                "network_enabled": meta.network.enabled,
                "compatibility": meta.compatibility,
                "resolved_packages": meta.resolved_packages,
                "allowed_tools": meta.allowed_tools,
                "path": skill_path.to_string_lossy(),
                "is_bash_tool": meta.is_bash_tool_skill(),
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
