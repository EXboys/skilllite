//! `skillbox init` — Initialize a project for SkillLite.
//!
//! Migrated from Python `skilllite init` command.
//!
//! Flow:
//!   1. Verify skillbox binary is available (self — always true)
//!   2. Create .skills/ directory + example skill (if empty)
//!   3. Scan all skills → resolve dependencies → install to isolated environments
//!   4. Run security audit (pip-audit / npm audit via dependency_audit)
//!   5. Output summary

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::metadata;

const EXAMPLE_SKILL_MD: &str = r#"---
name: hello-world
description: A simple example skill that greets the user
entry_point: main.py
language: python
---

# Hello World Skill

A minimal example skill to demonstrate SkillLite's structure.

## Usage

This skill takes a `name` parameter and returns a greeting.

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Name to greet"
    }
  }
}
```
"#;

const EXAMPLE_MAIN_PY: &str = r#"#!/usr/bin/env python3
"""Hello World skill — greets the user by name."""

import json
import sys


def main():
    data = json.load(sys.stdin)
    name = data.get("name", "World")
    result = {"greeting": f"Hello, {name}!"}
    print(json.dumps(result))


if __name__ == "__main__":
    main()
"#;

/// `skillbox init`
pub fn cmd_init(
    skills_dir: &str,
    skip_deps: bool,
    skip_audit: bool,
    strict: bool,
) -> Result<()> {
    let skills_path = resolve_path(skills_dir);

    eprintln!("🚀 Initializing SkillLite project...");
    eprintln!();

    // Step 1: Binary check (we ARE the binary)
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("✅ Step 1/5: skillbox binary v{} ready", version);

    // Step 2: Create .skills/ directory + example skill
    eprintln!();
    let created_example = create_skills_dir(&skills_path)?;
    if created_example {
        eprintln!("✅ Step 2/5: Created {} with example skill", skills_dir);
    } else {
        eprintln!("✅ Step 2/5: Skills directory already exists at {}", skills_dir);
    }

    // Step 3: Scan all skills and install dependencies
    eprintln!();
    let skills = discover_all_skills(&skills_path);
    if skills.is_empty() {
        eprintln!("✅ Step 3/5: No skills found to process");
    } else {
        eprintln!("📦 Step 3/5: Processing {} skill(s)...", skills.len());
        if skip_deps {
            eprintln!("   ⏭ Skipping dependency installation (--skip-deps)");
        } else {
            let dep_results = install_all_deps(&skills_path, &skills);
            for msg in &dep_results {
                eprintln!("{}", msg);
            }
        }
    }

    // Step 4: Security audit
    eprintln!();
    if skip_audit {
        eprintln!("✅ Step 4/5: Skipping security audit (--skip-audit)");
    } else {
        let (audit_msgs, has_vulns) = audit_all_skills(&skills_path, &skills);
        if audit_msgs.is_empty() {
            eprintln!("✅ Step 4/5: No dependencies to audit");
        } else {
            eprintln!("🔍 Step 4/5: Security audit results:");
            for msg in &audit_msgs {
                eprintln!("{}", msg);
            }
            if has_vulns && strict {
                anyhow::bail!(
                    "Security audit failed in strict mode. Fix vulnerabilities before proceeding.\n\
                     Run `skillbox dependency-audit <skill_dir>` for details."
                );
            }
        }
    }

    // Step 5: Summary
    eprintln!();
    eprintln!("✅ Step 5/5: Initialization complete!");
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

/// Create .skills/ directory and an example skill if it doesn't exist.
/// Returns true if example skill was created.
fn create_skills_dir(skills_path: &Path) -> Result<bool> {
    if skills_path.exists() {
        // Check if there are any skills already
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

    // Create example skill
    let example_dir = skills_path.join("hello-world");
    if !example_dir.exists() {
        fs::create_dir_all(&example_dir)
            .context("Failed to create example skill directory")?;
        fs::write(example_dir.join("SKILL.md"), EXAMPLE_SKILL_MD)
            .context("Failed to write example SKILL.md")?;
        fs::write(example_dir.join("main.py"), EXAMPLE_MAIN_PY)
            .context("Failed to write example main.py")?;
        eprintln!("   Created example skill: hello-world");
        return Ok(true);
    }

    Ok(false)
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
fn install_all_deps(skills_path: &Path, skills: &[String]) -> Vec<String> {
    let mut messages = Vec::new();

    for name in skills {
        let skill_path = skills_path.join(name);
        if !skill_path.join("SKILL.md").exists() {
            continue;
        }

        match metadata::parse_skill_metadata(&skill_path) {
            Ok(meta) => {
                if meta.entry_point.is_empty() && !meta.is_bash_tool_skill() {
                    messages.push(format!("   ✓ {} (prompt-only): no dependencies needed", name));
                    continue;
                }

                let lang = metadata::detect_language(&skill_path, &meta);
                let cache_dir: Option<&str> = None;
                match crate::env::builder::ensure_environment(&skill_path, &meta, cache_dir) {
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
            let scanner = crate::sandbox::security::ScriptScanner::new();
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
                                crate::sandbox::security::types::SecuritySeverity::High
                                    | crate::sandbox::security::types::SecuritySeverity::Critical
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
                use crate::sandbox::security::dependency_audit;
                match dependency_audit::audit_skill_dependencies(&skill_path) {
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
    eprintln!("   1. Add skills:    skillbox add owner/repo");
    eprintln!("   2. IDE setup:     skillbox init-cursor");
    eprintln!("   3. Start chat:    skillbox chat");
    eprintln!("   4. Or quickstart: skillbox quickstart");
    eprintln!("{}", "═".repeat(50));
}
