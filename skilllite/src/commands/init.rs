//! `skilllite init` — Initialize a project for SkillLite.
//!
//! Migrated from Python `skilllite init` command.
//!
//! Flow:
//!   1. Verify skilllite binary is available (self — always true)
//!   2. Create .skills/ directory + download skills from SKILLLITE_SKILLS_REPO (if empty)
//!   3. Scan all skills → resolve dependencies → install to isolated environments
//!   4. Run security audit (pip-audit / npm audit via dependency_audit)
//!   5. Output summary

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::skill;
use skilllite_core::skill::dependency_resolver;
use skilllite_core::skill::metadata;

/// `skilllite init`
pub fn cmd_init(
    skills_dir: &str,
    skip_deps: bool,
    skip_audit: bool,
    strict: bool,
    force: bool,
    use_llm: bool,
) -> Result<()> {
    let skills_path = resolve_path(skills_dir);

    eprintln!("🚀 Initializing SkillLite project...");
    eprintln!();

    // Step 1: Binary check (we ARE the binary)
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("✅ Step 1/6: skilllite binary v{} ready", version);

    // Step 2: Create .skills/ directory + download skills (if empty)
    eprintln!();
    let downloaded = ensure_skills_dir(&skills_path, force)?;
    if downloaded {
        eprintln!("✅ Step 2/6: Downloaded skills into {}", skills_dir);
    } else {
        eprintln!("✅ Step 2/6: Skills directory already exists at {}", skills_dir);
    }

    // Step 3: Scan all skills and install dependencies
    eprintln!();
    let skills = discover_all_skills(&skills_path);
    if skills.is_empty() {
        eprintln!("✅ Step 3/6: No skills found to process");
    } else {
        eprintln!("📦 Step 3/6: Processing {} skill(s)...", skills.len());
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

    // Step 4: Security audit
    eprintln!();
    if skip_audit {
        eprintln!("✅ Step 4/6: Skipping security audit (--skip-audit)");
    } else {
        let (audit_msgs, has_vulns) = audit_all_skills(&skills_path, &skills);
        if audit_msgs.is_empty() {
            eprintln!("✅ Step 4/6: No dependencies to audit");
        } else {
            eprintln!("🔍 Step 4/6: Security audit results:");
            for msg in &audit_msgs {
                eprintln!("{}", msg);
            }
            if has_vulns && strict {
                anyhow::bail!(
                    "Security audit failed in strict mode. Fix vulnerabilities before proceeding.\n\
                     Run `skilllite dependency-audit <skill_dir>` for details."
                );
            }
        }
    }

    // Step 5: Generate planning rules (when --use-llm and API key available)
    eprintln!();
    #[cfg(feature = "agent")]
    if skills.is_empty() {
        eprintln!("✅ Step 5/6: No skills, skipping planning rules");
    } else if !use_llm {
        eprintln!("✅ Step 5/6: Skipping planning rules (use --use-llm to generate)");
    } else {
        eprintln!("📋 Step 5/6: Generating planning rules...");
        let workspace = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        match crate::commands::planning_rules_gen::generate_planning_rules(
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
    eprintln!("✅ Step 5/6: Planning rules (requires agent feature)");

    // Step 6: Summary
    eprintln!();
    eprintln!("✅ Step 6/6: Initialization complete!");
    eprintln!();
    print_summary(&skills_path, &skills);

    Ok(())
}

fn resolve_path(dir: &str) -> PathBuf {
    let p = PathBuf::from(dir);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// Ensure .skills/ directory exists and has skills. When empty, download from
/// SKILLLITE_SKILLS_REPO (default: EXboys/skilllite). Returns true if skills were downloaded.
///
/// Shared by `init` and `quickstart` commands.
pub(crate) fn ensure_skills_dir(skills_path: &Path, force: bool) -> Result<bool> {
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

    fs::create_dir_all(skills_path)
        .with_context(|| format!("Failed to create skills directory: {}", skills_path.display()))?;

    let repo = skilllite_core::config::PathsConfig::from_env().skills_repo;
    let skills_dir_str = skills_path.to_string_lossy().to_string();

    eprintln!("   📥 Downloading skills from {} ...", repo);
    skill::cmd_add(&repo, &skills_dir_str, force, false)
        .with_context(|| format!("Failed to download skills from {}. Set SKILLLITE_SKILLS_REPO to customize.", repo))?;

    Ok(true)
}

/// Count skills in the directory (subdirs with SKILL.md). Used for status messages.
pub(crate) fn count_skills(skills_path: &Path) -> usize {
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
            let client = skilllite_agent::llm::LlmClient::new(&config.api_base, &config.api_key);
            (Some(client), Some(config.model))
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
                if meta.entry_point.is_empty() && !meta.is_bash_tool_skill() {
                    messages.push(format!("   ✓ {} (prompt-only): no dependencies needed", name));
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
                            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                            rt.block_on(skilllite_agent::dependency_resolver::resolve_packages(
                                &skill_path,
                                meta.compatibility.as_deref(),
                                &lang,
                                Some(client),
                                Some(m),
                                true,
                            ))
                        } else {
                            dependency_resolver::resolve_packages_sync(
                                &skill_path,
                                meta.compatibility.as_deref(),
                                &lang,
                                true,
                            )
                        }
                    };

                    #[cfg(not(feature = "agent"))]
                    let resolved =
                        dependency_resolver::resolve_packages_sync(
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
                match skilllite_sandbox::env::builder::ensure_environment(&skill_path, &meta, cache_dir) {
                    Ok(_) => {
                        messages.push(format!("   ✓ {} [{}]: dependencies installed", name, lang));
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
                    .map(|meta| dependency_audit::MetadataHint {
                        compatibility: meta.compatibility,
                        resolved_packages: meta.resolved_packages,
                        description: meta.description,
                        language: meta.language,
                        entry_point: meta.entry_point,
                    });
                match dependency_audit::audit_skill_dependencies(&skill_path, metadata_hint.as_ref()) {
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

fn print_summary(skills_path: &Path, skills: &[String]) {
    eprintln!("{}", "═".repeat(50));
    eprintln!("🎉 SkillLite project initialized!");
    eprintln!();
    eprintln!("   Skills directory: {}", skills_path.display());
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
            eprintln!("   • {}{}", name, if short.is_empty() { String::new() } else { format!(": {}", short) });
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
