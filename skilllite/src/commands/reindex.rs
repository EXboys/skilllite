//! Reindex command: rescan skills directory and rebuild metadata cache.
//!
//! Validates all SKILL.md files, checks dependencies, and reports status.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::skill::metadata;

/// `skilllite reindex`
pub fn cmd_reindex(skills_dir: &str, verbose: bool) -> Result<()> {
    let skills_path = if PathBuf::from(skills_dir).is_absolute() {
        PathBuf::from(skills_dir)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(skills_dir)
    };

    if !skills_path.exists() {
        eprintln!("Skills directory not found: {}", skills_path.display());
        eprintln!("Create it with: skilllite skill add <source>");
        return Ok(());
    }

    eprintln!("🔍 Reindexing skills in {} ...", skills_path.display());
    eprintln!();

    let mut total = 0;
    let mut valid = 0;
    let mut errors = 0;

    let mut entries: Vec<_> = fs::read_dir(&skills_path)
        .context("Failed to read skills directory")?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        if !p.join("SKILL.md").exists() {
            if verbose {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                eprintln!("  ⏭ {}: no SKILL.md (skipped)", name);
            }
            continue;
        }

        total += 1;
        match metadata::parse_skill_metadata(&p) {
            Ok(meta) => {
                valid += 1;
                let lang = metadata::detect_language(&p, &meta);
                let skill_type = if meta.is_bash_tool_skill() {
                    "bash-tool"
                } else if meta.entry_point.is_empty() {
                    "prompt-only"
                } else {
                    "standard"
                };
                let net = if meta.network.enabled { "🌐" } else { "" };
                let has_lock = if p.join(".skilllite.lock").exists() {
                    "🔒"
                } else {
                    ""
                };

                eprintln!(
                    "  ✓ {} [{}] ({}) {} {}",
                    meta.name, lang, skill_type, net, has_lock
                );

                if verbose {
                    if let Some(ref desc) = meta.description {
                        eprintln!("      {}", desc);
                    }
                    if !meta.entry_point.is_empty() {
                        eprintln!("      entry: {}", meta.entry_point);
                    }
                    if let Some(ref pkgs) = meta.resolved_packages {
                        eprintln!("      packages: {}", pkgs.join(", "));
                    }
                }
            }
            Err(e) => {
                errors += 1;
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                eprintln!("  ✗ {}: {}", name, e);
            }
        }
    }

    eprintln!();
    eprintln!("Summary: {} skill(s) scanned, {} valid, {} error(s)", total, valid, errors);

    if errors > 0 {
        eprintln!("⚠ Fix errors in SKILL.md files above to ensure proper functionality.");
    }

    Ok(())
}
