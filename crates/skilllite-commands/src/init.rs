//! `skilllite init` — Initialize a project for SkillLite.
//!
//! Migrated from Python `skilllite init` command.
//!
//! Flow:
//!   1. Verify skilllite binary is available (self — always true)
//!   2. Create skills/ directory + download skills from SKILLLITE_SKILLS_REPO (if empty)
//!   3. Create project Repo Wiki markdown skeleton under .skilllite/wiki/
//!   4. Scan all skills → resolve dependencies → install to isolated environments
//!   5. Run security audit (pip-audit / npm audit via dependency_audit)
//!   6. Generate planning rules when requested
//!   7. Output summary

use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::bail;
use crate::skill;
use crate::Result;
use skilllite_core::paths;
use skilllite_core::skill::dependency_resolver;
use skilllite_core::skill::discovery;
use skilllite_core::skill::metadata;

/// True when `cwd` is a typical GUI / launcher default (not a project dir). Relative `skills_dir`
/// would resolve under it and often hit permission errors or wrong location.
fn cwd_is_untrusted_for_relative_skills(cwd: &Path) -> bool {
    #[cfg(unix)]
    {
        cwd == Path::new("/")
    }
    #[cfg(windows)]
    {
        windows_cwd_untrusted_for_relative_skills(cwd)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = cwd;
        false
    }
}

#[cfg(windows)]
fn windows_cwd_untrusted_for_relative_skills(cwd: &Path) -> bool {
    let lower = cwd.to_string_lossy().to_ascii_lowercase();
    // Double-clicked / service-style defaults — not a repo root.
    if lower.contains("\\windows\\system32") {
        return true;
    }
    if lower.contains("\\program files\\")
        || lower.ends_with("\\program files")
        || lower.contains("\\program files (x86)\\")
        || lower.ends_with("\\program files (x86)")
    {
        return true;
    }
    if let Ok(sr) = std::env::var("SYSTEMROOT") {
        let sr = Path::new(&sr);
        if let (Ok(c), Ok(s)) = (cwd.canonicalize(), sr.canonicalize()) {
            let sys32 = s.join("System32");
            if let Ok(s32) = sys32.canonicalize() {
                if c == s32 || c.starts_with(&s32) {
                    return true;
                }
            }
        }
    }
    false
}

/// When the process cwd is untrusted (e.g. `/` on macOS GUI, `System32` on Windows) and
/// `skills_dir` is relative, resolving `skills` would land in a bad path (read-only or wrong tree).
pub(crate) fn reject_relative_skills_dir_when_cwd_root(skills_dir: &str) -> Result<()> {
    if Path::new(skills_dir).is_absolute() {
        return Ok(());
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd_is_untrusted_for_relative_skills(&cwd) {
        bail!(
            "当前工作目录 {:?} 不适合解析相对 skills 路径 {:?}（将写入 {}）。\
             请先在项目目录下打开终端再执行，或使用：skilllite init -s <skills 目录的绝对路径>",
            cwd,
            skills_dir,
            cwd.join(skills_dir).display()
        );
    }
    Ok(())
}

/// `skilllite init`
pub fn cmd_init(
    skills_dir: &str,
    skip_deps: bool,
    skip_audit: bool,
    strict: bool,
    force: bool,
    use_llm: bool,
) -> Result<()> {
    reject_relative_skills_dir_when_cwd_root(skills_dir)?;
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let skills_path = resolve_path_with_legacy_fallback(skills_dir);

    eprintln!("🚀 Initializing SkillLite project...");
    eprintln!();

    // Step 1: Binary check (we ARE the binary)
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("✅ Step 1/7: skilllite binary v{} ready", version);

    // Step 2: Create skills/ directory + download skills (if empty)
    eprintln!();
    let downloaded = ensure_skills_dir(&skills_path, force)?;
    if downloaded {
        eprintln!(
            "✅ Step 2/7: Downloaded skills into {}",
            skills_path.display()
        );
    } else {
        eprintln!(
            "✅ Step 2/7: Skills directory already exists at {}",
            skills_path.display()
        );
    }

    // Step 3: Create project Repo Wiki skeleton
    eprintln!();
    let wiki_path = ensure_project_wiki(&workspace)?;
    eprintln!(
        "✅ Step 3/7: Project Repo Wiki ready at {}",
        wiki_path.display()
    );

    // Step 4: Scan all skills and install dependencies
    eprintln!();
    let skills = discover_all_skills(&skills_path);
    if skills.is_empty() {
        eprintln!("✅ Step 4/7: No skills found to process");
    } else {
        eprintln!("📦 Step 4/7: Processing {} skill(s)...", skills.len());
        if skip_deps {
            eprintln!("   ⏭ Skipping dependency installation (--skip-deps)");
        } else {
            if force {
                eprintln!("   🔄 --force: re-resolving dependencies (ignoring .skilllite.lock)");
            }
            let dep_results = install_all_deps(&skills_path, &skills, force, use_llm);
            for msg in &dep_results {
                eprintln!("{}", msg);
            }
        }
    }

    // Step 5: Security audit
    eprintln!();
    if skip_audit {
        eprintln!("✅ Step 5/7: Skipping security audit (--skip-audit)");
    } else {
        let (audit_msgs, has_vulns) = audit_all_skills(&skills_path, &skills);
        if audit_msgs.is_empty() {
            eprintln!("✅ Step 5/7: No dependencies to audit");
        } else {
            eprintln!("🔍 Step 5/7: Security audit results:");
            for msg in &audit_msgs {
                eprintln!("{}", msg);
            }
            if has_vulns && strict {
                bail!(
                    "Security audit failed in strict mode. Fix vulnerabilities before proceeding.\n\
                     Run `skilllite dependency-audit <skill_dir>` for details."
                );
            }
        }
    }

    // Step 6: Generate planning rules (when --use-llm and API key available)
    eprintln!();
    #[cfg(feature = "agent")]
    if skills.is_empty() {
        eprintln!("✅ Step 6/7: No skills, skipping planning rules");
    } else if !use_llm {
        eprintln!("✅ Step 6/7: Skipping planning rules (use --use-llm to generate)");
    } else {
        eprintln!("📋 Step 6/7: Generating planning rules...");
        match crate::planning_rules_gen::generate_planning_rules(
            &workspace,
            &skills_path,
            &skills,
            true,
        ) {
            Ok(path) => {
                eprintln!("   ✅ Saved to {}", path.display());
            }
            Err(e) => {
                eprintln!("   ⚠ Skipped ({})", e);
            }
        }
    }
    #[cfg(not(feature = "agent"))]
    eprintln!("✅ Step 6/7: Planning rules (requires agent feature)");

    // Step 7: Summary
    eprintln!();
    eprintln!("✅ Step 7/7: Initialization complete!");
    eprintln!();
    print_summary(&skills_path, &wiki_path, &skills);

    Ok(())
}

pub(crate) fn resolve_path_with_legacy_fallback(dir: &str) -> PathBuf {
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let resolution = discovery::resolve_skills_dir_with_legacy_fallback(&workspace, dir);
    if let Some(warning) = resolution.conflict_warning() {
        eprintln!("{}", warning);
    }
    resolution.effective_path
}

/// Ensure skills directory exists and has skills. When empty, download from
/// SKILLLITE_SKILLS_REPO (default: EXboys/skilllite). Returns true if skills were downloaded.
///
/// Shared by `init` and `quickstart` commands.
pub fn ensure_skills_dir(skills_path: &Path, force: bool) -> Result<bool> {
    if skills_path.exists() {
        let has_skills = fs::read_dir(skills_path)
            .map(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
            })
            .unwrap_or(false);
        if has_skills {
            return Ok(false);
        }
    }

    fs::create_dir_all(skills_path).with_context(|| {
        format!(
            "Failed to create skills directory: {}",
            skills_path.display()
        )
    })?;

    let repo = skilllite_core::config::PathsConfig::from_env().skills_repo;
    let skills_dir_str = skills_path.to_string_lossy().to_string();

    eprintln!("   📥 Downloading skills from {} ...", repo);
    skill::cmd_add(&repo, &skills_dir_str, force, false, false).with_context(|| {
        format!(
            "Failed to download skills from {}. Set SKILLLITE_SKILLS_REPO to customize.",
            repo
        )
    })?;

    Ok(true)
}

pub fn ensure_project_wiki(workspace: &Path) -> Result<PathBuf> {
    let wiki_path = paths::project_wiki_root(workspace);
    fs::create_dir_all(wiki_path.join("raw")).with_context(|| {
        format!(
            "Failed to create project wiki raw directory: {}",
            wiki_path.join("raw").display()
        )
    })?;
    fs::create_dir_all(wiki_path.join("wiki")).with_context(|| {
        format!(
            "Failed to create project wiki article directory: {}",
            wiki_path.join("wiki").display()
        )
    })?;
    fs::create_dir_all(wiki_path.join("output")).with_context(|| {
        format!(
            "Failed to create project wiki output directory: {}",
            wiki_path.join("output").display()
        )
    })?;
    fs::create_dir_all(wiki_path.join("lessons")).with_context(|| {
        format!(
            "Failed to create project wiki lessons directory: {}",
            wiki_path.join("lessons").display()
        )
    })?;

    write_if_missing(
        &wiki_path.join("_index.md"),
        "# SkillLite Repo Wiki\n\nProject-local Markdown knowledge base for this repository.\n\n## Contents\n\n- [Config](config.md)\n- [Raw sources](raw/)\n- [Compiled wiki](wiki/)\n- [Lessons](lessons/)\n- [Output](output/)\n",
    )?;
    write_if_missing(
        &wiki_path.join("config.md"),
        "# Repo Wiki Config\n\n- Scope: project-local SkillLite Repo Wiki\n- Storage: plain Markdown\n- SQLite: not used\n",
    )?;
    write_if_missing(
        &wiki_path.join("log.md"),
        "# Repo Wiki Log\n\nAppend wiki operations here.\n",
    )?;

    Ok(wiki_path)
}

fn write_if_missing(path: &Path, content: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    Ok(())
}

/// Count skills in the directory (subdirs with SKILL.md). Used for status messages.
pub fn count_skills(skills_path: &Path) -> usize {
    discover_all_skills(skills_path).len()
}

/// Discover all skills in the skills directory.
fn discover_all_skills(skills_path: &Path) -> Vec<String> {
    let mut skills = Vec::new();
    if !skills_path.is_dir() {
        return skills;
    }

    if let Ok(entries) = fs::read_dir(skills_path) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    skills.push(name.to_string());
                }
            }
        }
    }

    skills
}

/// Install dependencies for all skills.
fn install_all_deps(
    skills_path: &Path,
    skills: &[String],
    force: bool,
    use_llm: bool,
) -> Vec<String> {
    let mut messages = Vec::new();

    #[cfg(feature = "agent")]
    let (llm_client, model) = if use_llm {
        let config = skilllite_agent::types::AgentConfig::from_env();
        if config.api_key.is_empty() {
            tracing::debug!("--use-llm requested but no API key; falling back to whitelist");
            (None, None)
        } else {
            match skilllite_agent::llm::LlmClient::new(&config.api_base, &config.api_key) {
                Ok(client) => (Some(client), Some(config.model)),
                Err(e) => {
                    tracing::warn!("LLM client build failed, falling back to whitelist: {}", e);
                    (None, None)
                }
            }
        }
    } else {
        (None, None)
    };

    #[cfg(not(feature = "agent"))]
    let _ = use_llm;

    for name in skills {
        let skill_path = skills_path.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }

        match metadata::parse_skill_metadata(&skill_path) {
            Ok(mut meta) => {
                if meta.entry_point.is_empty()
                    && !meta.is_bash_tool_skill()
                    && !metadata::has_executable_scripts(&skill_path)
                {
                    messages.push(format!(
                        "   ✓ {} (prompt-only): no dependencies needed",
                        name
                    ));
                    continue;
                }

                let lang = metadata::detect_language(&skill_path, &meta);

                // Resolve dependencies when lock is missing/stale, or --force.
                // With --use-llm: Lock → LLM → Whitelist. Otherwise: Lock → Whitelist.
                if meta.resolved_packages.is_none() || force {
                    #[cfg(feature = "agent")]
                    let resolved = {
                        let client_opt = llm_client.as_ref();
                        let model_opt = model.as_deref();
                        if let (Some(client), Some(m)) = (client_opt, model_opt) {
                            match tokio::runtime::Runtime::new() {
                                Ok(rt) => rt
                                    .block_on(
                                        skilllite_agent::dependency_resolver::resolve_packages(
                                            &skill_path,
                                            meta.compatibility.as_deref(),
                                            &lang,
                                            Some(client),
                                            Some(m),
                                            true,
                                        ),
                                    )
                                    .map_err(anyhow::Error::from),
                                Err(e) => {
                                    tracing::warn!(skill = %name, err = %e, "tokio runtime failed, skipping LLM dependency resolution");
                                    dependency_resolver::resolve_packages_sync(
                                        &skill_path,
                                        meta.compatibility.as_deref(),
                                        &lang,
                                        true,
                                    )
                                    .map_err(anyhow::Error::from)
                                }
                            }
                        } else {
                            dependency_resolver::resolve_packages_sync(
                                &skill_path,
                                meta.compatibility.as_deref(),
                                &lang,
                                true,
                            )
                            .map_err(anyhow::Error::from)
                        }
                    };

                    #[cfg(not(feature = "agent"))]
                    let resolved = dependency_resolver::resolve_packages_sync(
                        &skill_path,
                        meta.compatibility.as_deref(),
                        &lang,
                        true,
                    );

                    if let Ok(resolved) = resolved {
                        if !resolved.packages.is_empty() {
                            tracing::debug!(
                                "{}: resolved {} packages via {}",
                                name,
                                resolved.packages.len(),
                                resolved.resolver
                            );
                            if !resolved.unknown_packages.is_empty() {
                                tracing::warn!(
                                    "{}: packages not in whitelist: {:?}",
                                    name,
                                    resolved.unknown_packages
                                );
                            }
                            meta.resolved_packages = Some(resolved.packages);
                        }
                    }
                }

                let cache_dir: Option<&str> = None;
                let env_spec = skilllite_core::EnvSpec::from_metadata(&skill_path, &meta);
                match skilllite_sandbox::env::builder::ensure_environment(
                    &skill_path,
                    &env_spec,
                    cache_dir,
                    None,
                    skilllite_sandbox::cli_confirm_download(),
                ) {
                    Ok(_) => {
                        messages.push(format!(
                            "   ✓ {} [{}]: dependencies installed",
                            name, env_spec.language
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

/// Run security audit on all skills.
/// Returns (messages, has_vulnerabilities).
fn audit_all_skills(skills_path: &Path, skills: &[String]) -> (Vec<String>, bool) {
    let mut messages = Vec::new();
    let mut has_vulns = false;

    for name in skills {
        let skill_path = skills_path.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }

        // Code security scan
        let meta = match metadata::parse_skill_metadata(&skill_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Collect scannable scripts
        let script_files = collect_script_files_for_audit(&skill_path, &meta);

        if !script_files.is_empty() {
            let scanner = skilllite_sandbox::security::ScriptScanner::new();
            let mut total_issues = 0usize;
            let mut total_high = 0usize;

            for script in &script_files {
                if let Ok(result) = scanner.scan_file(script) {
                    let high = result
                        .issues
                        .iter()
                        .filter(|i| {
                            matches!(
                                i.severity,
                                skilllite_sandbox::security::types::SecuritySeverity::High
                                    | skilllite_sandbox::security::types::SecuritySeverity::Critical
                            )
                        })
                        .count();
                    total_issues += result.issues.len();
                    total_high += high;
                }
            }

            if total_issues > 0 {
                if total_high > 0 {
                    has_vulns = true;
                }
                messages.push(format!(
                    "   🔒 {} code: {} issue(s) ({} high/critical)",
                    name, total_issues, total_high
                ));
            } else {
                messages.push(format!("   🔒 {} code: ✅ clean", name));
            }
        }

        // Supply chain audit
        #[cfg(feature = "audit")]
        {
            let has_deps = skill_path.join("requirements.txt").exists()
                || skill_path.join("package.json").exists();

            if has_deps {
                use skilllite_sandbox::security::dependency_audit;
                let metadata_hint = metadata::parse_skill_metadata(&skill_path)
                    .ok()
                    .map(|m| crate::security::metadata_hint_from_skill_metadata(&m));
                match dependency_audit::audit_skill_dependencies(
                    &skill_path,
                    metadata_hint.as_ref(),
                ) {
                    Ok(result) => {
                        if result.vulnerable_count > 0 {
                            has_vulns = true;
                            messages.push(format!(
                                "   🛡 {} deps: ⚠ {}/{} vulnerable ({} vulns)",
                                name, result.vulnerable_count, result.scanned, result.total_vulns
                            ));
                        } else if result.scanned > 0 {
                            messages.push(format!(
                                "   🛡 {} deps: ✅ {} packages clean",
                                name, result.scanned
                            ));
                        }
                    }
                    Err(e) => {
                        messages.push(format!("   🛡 {} deps: ⚠ error: {}", name, e));
                    }
                }
            }
        }
    }

    (messages, has_vulns)
}

fn collect_script_files_for_audit(
    skill_path: &Path,
    meta: &metadata::SkillMetadata,
) -> Vec<PathBuf> {
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

    // Scripts directory
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
                let is_script = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| matches!(ext, "py" | "js" | "ts" | "sh"))
                    .unwrap_or(false);
                if !is_script {
                    continue;
                }
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with("test_")
                    || name.ends_with("_test.py")
                    || name == "__init__.py"
                    || name.starts_with('.')
                {
                    continue;
                }
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical) {
                    files.push(path);
                }
            }
        }
    }

    files
}

fn print_summary(skills_path: &Path, wiki_path: &Path, skills: &[String]) {
    eprintln!("{}", "═".repeat(50));
    eprintln!("🎉 SkillLite project initialized!");
    eprintln!();
    eprintln!("   Skills directory: {}", skills_path.display());
    eprintln!("   Repo Wiki: {}", wiki_path.display());
    eprintln!("   Skills found: {}", skills.len());

    if !skills.is_empty() {
        eprintln!();
        for name in skills {
            let skill_path = skills_path.join(name);
            let desc = metadata::parse_skill_metadata(&skill_path)
                .ok()
                .and_then(|m| m.description)
                .unwrap_or_default();
            let short: String = desc.chars().take(50).collect();
            eprintln!(
                "   • {}{}",
                name,
                if short.is_empty() {
                    String::new()
                } else {
                    format!(": {}", short)
                }
            );
        }
    }

    eprintln!();
    eprintln!("Next steps:");
    eprintln!("   1. Add skills:    skilllite add owner/repo");
    eprintln!("   2. IDE setup:     skilllite init-cursor");
    eprintln!("   3. Start chat:    skilllite chat");
    eprintln!("   4. Or quickstart: skilllite quickstart");
    eprintln!("{}", "═".repeat(50));
}

#[cfg(test)]
mod cwd_trust_tests {
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    #[test]
    fn unix_root_is_untrusted_for_relative_skills() {
        assert!(super::cwd_is_untrusted_for_relative_skills(Path::new("/")));
    }

    #[cfg(unix)]
    #[test]
    fn unix_projectish_dir_is_trusted() {
        assert!(!super::cwd_is_untrusted_for_relative_skills(Path::new(
            "/tmp"
        )));
    }

    #[test]
    fn ensure_project_wiki_creates_markdown_skeleton_without_sqlite() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!(
            "skilllite-wiki-test-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&workspace).expect("create temp workspace");

        let wiki = super::ensure_project_wiki(&workspace).expect("create project wiki");

        assert_eq!(wiki, workspace.join(".skilllite").join("wiki"));
        assert!(wiki.join("_index.md").is_file());
        assert!(wiki.join("config.md").is_file());
        assert!(wiki.join("log.md").is_file());
        assert!(wiki.join("raw").is_dir());
        assert!(wiki.join("wiki").is_dir());
        assert!(wiki.join("output").is_dir());
        assert!(wiki.join("lessons").is_dir());
        assert!(!wiki.join(".index").exists());
        assert!(!wiki.join("default.sqlite").exists());

        std::fs::remove_dir_all(&workspace).expect("remove temp workspace");
    }
}
