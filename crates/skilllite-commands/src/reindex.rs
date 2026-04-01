//! Reindex command: rescan skills directory and rebuild metadata cache.
//!
//! Validates all SKILL.md files, checks dependencies, and reports status.

use anyhow::Context;
use std::fs;

use skilllite_core::skill::manifest;
use skilllite_core::skill::metadata;

use crate::Result;

/// `skilllite reindex`
pub fn cmd_reindex(skills_dir: &str, verbose: bool, rebuild_manifest: bool) -> Result<()> {
    let skills_path = crate::init::resolve_path_with_legacy_fallback(skills_dir);

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
    let mut manifest_rebuilt = 0;

    let existing_manifest = if rebuild_manifest {
        Some(manifest::load_manifest(&skills_path).unwrap_or_default())
    } else {
        None
    };

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
                } else if meta.entry_point.is_empty() && metadata::has_executable_scripts(&p) {
                    "script-no-default-entry"
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

                if rebuild_manifest {
                    let skill_name = if meta.name.is_empty() {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    } else {
                        meta.name.clone()
                    };
                    let previous_source = existing_manifest
                        .as_ref()
                        .and_then(|m| m.skills.get(&skill_name))
                        .map(|e| e.source.clone())
                        .unwrap_or_else(|| "reindex-local".to_string());
                    manifest::upsert_installed_skill(&skills_path, &p, &previous_source)?;
                    manifest_rebuilt += 1;
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
    eprintln!(
        "Summary: {} skill(s) scanned, {} valid, {} error(s)",
        total, valid, errors
    );
    if rebuild_manifest {
        eprintln!(
            "Manifest: rebuilt/updated {} skill entr{}",
            manifest_rebuilt,
            if manifest_rebuilt == 1 { "y" } else { "ies" }
        );
    }

    if errors > 0 {
        eprintln!("⚠ Fix errors in SKILL.md files above to ensure proper functionality.");
    }

    Ok(())
}
