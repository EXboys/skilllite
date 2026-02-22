//! Skill management commands: add, remove, list, show.
//!
//! Migrated from Python `python-sdk/skilllite/cli/add.py` and `repo.py`.
//! Depends ONLY on skill/ and env/ layers (Layer 1-2), NOT on agent/ (Layer 3).

use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use skilllite_sandbox::security::ScriptScanner;
use skilllite_core::skill::metadata;

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
    // ClawHub: clawhub:<skill-name>
    if let Some(slug) = source.strip_prefix("clawhub:") {
        let slug = slug.trim().to_lowercase();
        if !slug.is_empty() {
            return ParsedSource {
                source_type: "clawhub".into(),
                url: slug,
                git_ref: None,
                subpath: None,
                skill_filter: None,
            };
        }
    }

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

// ─── ClawHub Download ──────────────────────────────────────────────────────

const CLAWHUB_DOWNLOAD_URL: &str = "https://clawhub.ai/api/v1/download";

#[cfg(feature = "audit")]
fn fetch_from_clawhub(slug: &str) -> Result<PathBuf> {
    use std::io::Read;

    let url = format!("{}?slug={}", CLAWHUB_DOWNLOAD_URL, slug);
    let agent = ureq::AgentBuilder::new().build();
    let resp = agent
        .get(&url)
        .call()
        .context("Failed to fetch from ClawHub. Check network.")?;

    let status = resp.status();
    if status != 200 {
        let body = resp.into_string().unwrap_or_default();
        anyhow::bail!(
            "ClawHub returned {} for slug '{}'. {}",
            status,
            slug,
            if body.len() > 200 { "" } else { &body }
        );
    }

    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .context("Failed to read zip from ClawHub")?;

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    #[allow(deprecated)]
    let extract_path = temp_dir.into_path();

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .context("Invalid zip from ClawHub")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let name = file.name().to_string();
        // Skip _meta.json and path traversal
        if name.contains("..") || name.starts_with('/') {
            continue;
        }
        let out_path = extract_path.join(&name);
        if file.is_dir() {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out_file = fs::File::create(&out_path)
                .with_context(|| format!("Failed to create {}", out_path.display()))?;
            std::io::copy(&mut file, &mut out_file)
                .with_context(|| format!("Failed to extract {}", name))?;
        }
    }

    Ok(extract_path)
}

#[cfg(not(feature = "audit"))]
fn fetch_from_clawhub(_slug: &str) -> Result<PathBuf> {
    anyhow::bail!("ClawHub download requires the 'audit' feature (ureq).")
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

    // repo_dir itself is a skill (e.g. ClawHub zip with SKILL.md at root)
    if repo_dir.join("SKILL.md").exists() {
        return vec![repo_dir.to_path_buf()];
    }

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

/// Files and directories to exclude when copying skills.
const COPY_EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    "venv",
    ".venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "dist",
    "build",
    "*.egg-info",
];

const COPY_EXCLUDE_FILES: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
];

const COPY_EXCLUDE_EXTENSIONS: &[&str] = &[
    "pyc",
    "pyo",
];

fn copy_dir_filtered(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create directory: {}", dest.display()))?;
    for entry in fs::read_dir(src)?.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip excluded directories
        if COPY_EXCLUDE_DIRS.iter().any(|d| {
            if d.contains('*') {
                let prefix = d.trim_end_matches('*').trim_end_matches('.');
                name_str.ends_with(prefix) || name_str.starts_with(prefix)
            } else {
                name_str == *d
            }
        }) && entry.path().is_dir()
        {
            continue;
        }

        // Skip excluded files
        if COPY_EXCLUDE_FILES.contains(&name_str.as_ref()) {
            continue;
        }

        // Skip excluded extensions
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if COPY_EXCLUDE_EXTENSIONS.contains(&ext) {
                continue;
            }
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
                match skilllite_sandbox::env::builder::ensure_environment(&skill_path, &meta, cache_dir) {
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

// ─── Security Scanning (on add) ─────────────────────────────────────────────

/// Collect all scannable script files from a skill directory.
///
/// Sources:
/// 1. Entry point (if declared)
/// 2. All .py / .js / .ts / .sh files in `scripts/` directory
/// Deduplicates so the entry point isn't scanned twice.
fn collect_script_files(skill_path: &Path, meta: &metadata::SkillMetadata) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Entry point
    if !meta.entry_point.is_empty() {
        let ep = skill_path.join(&meta.entry_point);
        if ep.exists() {
            if let Ok(canonical) = ep.canonicalize() {
                seen.insert(canonical);
            }
            files.push(ep);
        }
    }

    // All scripts in scripts/ directory
    let scripts_dir = skill_path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&scripts_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let dominated = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| matches!(ext, "py" | "js" | "ts" | "sh"))
                    .unwrap_or(false);
                if !dominated {
                    continue;
                }
                // Skip test files and __init__.py
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with("test_")
                    || name.ends_with("_test.py")
                    || name == "__init__.py"
                    || name.starts_with('.')
                {
                    continue;
                }
                // Deduplicate (entry point might be in scripts/)
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical) {
                    files.push(path);
                }
            }
        }
    }

    files
}

/// Run security scans on newly installed skills:
/// 1. Static code analysis on all script files
/// 2. Supply chain vulnerability audit on dependencies (requires `audit` feature)
///
/// Returns (messages, has_high_risk) where has_high_risk is true if any
/// high/critical code issues or vulnerable dependencies were found.
fn scan_installed_skills(skills_dir: &Path, installed: &[String]) -> (Vec<String>, bool) {
    let mut messages = Vec::new();
    let mut has_high_risk = false;

    for name in installed {
        let skill_path = skills_dir.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }

        // ── Code security scan ──
        let meta = match metadata::parse_skill_metadata(&skill_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // ── Collect all scannable scripts ──
        let script_files = collect_script_files(&skill_path, &meta);

        // Check if there are dependencies (explicit files, lock, or inferrable from metadata)
        let has_deps = skill_path.join("requirements.txt").exists()
            || skill_path.join("package.json").exists()
            || skill_path.join(".skilllite.lock").exists()
            || meta.resolved_packages.is_some()
            || meta.compatibility.as_ref().map_or(false, |c| !c.is_empty());

        // Nothing to scan at all
        if script_files.is_empty() && !has_deps {
            let skill_type = if meta.is_bash_tool_skill() {
                "bash-tool"
            } else {
                "prompt-only"
            };
            messages.push(format!(
                "   ✅ {} ({}): no scripts or dependencies to scan",
                name, skill_type
            ));
            continue;
        }

        // ── Code security scan (all scripts) ──
        if !script_files.is_empty() {
            let scanner = ScriptScanner::new();
            let mut total_issues = 0usize;
            let mut total_high = 0usize;
            let mut worst_file: Option<String> = None;

            for script_path in &script_files {
                if let Ok(result) = scanner.scan_file(script_path) {
                    let high = result.issues.iter().filter(|i| {
                        matches!(
                            i.severity,
                            skilllite_sandbox::security::types::SecuritySeverity::High
                                | skilllite_sandbox::security::types::SecuritySeverity::Critical
                        )
                    }).count();
                    total_issues += result.issues.len();
                    total_high += high;
                    if high > 0 && worst_file.is_none() {
                        worst_file = Some(script_path.display().to_string());
                    }
                }
            }

            if total_issues > 0 {
                if total_high > 0 {
                    has_high_risk = true;
                }
                messages.push(format!(
                    "   🔒 {} code scan: {} issue(s) across {} file(s) ({} high/critical)",
                    name, total_issues, script_files.len(), total_high
                ));
                if let Some(ref path) = worst_file {
                    messages.push(format!(
                        "      ⚠ Run `skilllite security-scan {}` for details",
                        path
                    ));
                }
            } else {
                messages.push(format!(
                    "   🔒 {} code scan: ✅ {} file(s) clean",
                    name, script_files.len()
                ));
            }
        }

        // ── Supply chain audit ──
        #[cfg(feature = "audit")]
        if has_deps {
            use skilllite_sandbox::security::dependency_audit;

            match dependency_audit::audit_skill_dependencies(&skill_path) {
                Ok(result) => {
                    if result.vulnerable_count > 0 {
                        has_high_risk = true;
                        messages.push(format!(
                            "   🛡 {} dependency audit: ⚠ {}/{} packages vulnerable ({} vulns)",
                            name, result.vulnerable_count, result.scanned, result.total_vulns
                        ));
                        for entry in result.entries.iter().filter(|e| !e.vulns.is_empty()).take(3) {
                            let vuln_ids: Vec<_> = entry.vulns.iter().take(2).map(|v| v.id.as_str()).collect();
                            let more = if entry.vulns.len() > 2 {
                                format!(" +{}", entry.vulns.len() - 2)
                            } else {
                                String::new()
                            };
                            messages.push(format!(
                                "      - {} {}: {}{}",
                                entry.name, entry.version, vuln_ids.join(", "), more
                            ));
                        }
                        messages.push(format!(
                            "      Run `skilllite dependency-audit {}` for full report",
                            skill_path.display()
                        ));
                    } else if result.scanned > 0 {
                        messages.push(format!(
                            "   🛡 {} dependency audit: ✅ {} packages clean",
                            name, result.scanned
                        ));
                    }
                }
                Err(e) => {
                    messages.push(format!(
                        "   🛡 {} dependency audit: ⚠ error: {}",
                        name, e
                    ));
                }
            }
        }
    }

    (messages, has_high_risk)
}

// ═══════════════════════════════════════════════════════════════════════════
// Public command handlers
// ═══════════════════════════════════════════════════════════════════════════

/// `skilllite skill add <source>`
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

        // ── Step 1: Security scans BEFORE installing dependencies ──
        // This is critical: pip install / npm install can execute arbitrary
        // code (setup.py, postinstall scripts). Scan first, warn early.
        eprintln!();
        eprintln!("🔍 Running security scans (pre-install)...");
        let (scan_messages, has_high_risk) = scan_installed_skills(&skills_path, &installed);
        for msg in &scan_messages {
            eprintln!("{}", msg);
        }

        // ── Step 2: If high-risk issues found, ask for confirmation ──
        if has_high_risk && !force {
            eprintln!();
            eprintln!("⚠️  High-risk issues detected. Installing dependencies may execute untrusted code.");
            eprint!("   Continue with dependency installation? [y/N] ");
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
                eprintln!("   Cancelled. Skills copied but dependencies NOT installed.");
                eprintln!("   You can review the code and run `skilllite dependency-audit <skill_dir>` manually.");
                return Ok(());
            }
        }

        // ── Step 3: Install dependencies (only after scan approval) ──
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

/// `skilllite skill remove <name>`
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

/// `skilllite skill list`
pub fn cmd_list(skills_dir: &str, json_output: bool) -> Result<()> {
    let skills_path = resolve_skills_dir(skills_dir);

    if !skills_path.exists() {
        if json_output {
            println!("[]");
        } else {
            eprintln!("No skills directory found. Run `skilllite skill add` first.");
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

/// `skilllite skill show <name>`
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
            let name = if meta.name.is_empty() {
                skill_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                meta.name.clone()
            };

            // Multi-script tools: when no entry_point, detect scripts and their schemas
            let multi_script_tools = if meta.entry_point.is_empty() && !meta.is_bash_tool_skill() {
                let tools = skilllite_core::skill::schema::detect_multi_script_tools(skill_path, &name);
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
