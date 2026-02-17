//! IDE integration commands: cursor, opencode.
//!
//! Migrated from Python `python-sdk/skilllite/cli/integrations/`.

use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::skill::metadata;

// ─── Shared Helpers ─────────────────────────────────────────────────────────

/// Scan skills directory and return basic info for each skill.
fn get_available_skills(skills_dir: &Path) -> Vec<(String, String)> {
    let mut skills = Vec::new();
    if !skills_dir.is_dir() {
        return skills;
    }
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return skills;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() || !p.join("SKILL.md").exists() {
            continue;
        }
        match metadata::parse_skill_metadata(&p) {
            Ok(meta) => {
                let desc = meta
                    .description
                    .unwrap_or_else(|| format!("Execute {} skill", meta.name));
                skills.push((meta.name, desc));
            }
            Err(_) => continue,
        }
    }
    skills.sort_by(|a, b| a.0.cmp(&b.0));
    skills
}

/// Detect the best command to start the MCP server.
/// Returns (command_args, description).
///
/// Priority:
///   1. `skilllite mcp-serve` (Rust native — fastest, no Python needed)
///   2. `uvx skilllite mcp` (Python SDK via uvx)
///   3. `skilllite mcp` (Python SDK in PATH)
///   4. `python3 -m skilllite.mcp.server` (fallback)
fn detect_best_mcp_command() -> (Vec<String>, String) {
    // Priority 1: skilllite native MCP server (Rust)
    if which("skilllite") {
        return (
            vec!["skilllite".into(), "mcp".into()],
            "skilllite (Rust native MCP)".into(),
        );
    }

    // Also check current binary path as fallback for skilllite
    if let Ok(exe) = std::env::current_exe() {
        if exe.exists() {
            let exe_str = exe.to_string_lossy().to_string();
            return (
                vec![exe_str, "mcp".into()],
                "skilllite (current binary)".into(),
            );
        }
    }

    // Priority 2: uvx (auto-managed Python)
    if which("uvx") {
        return (
            vec!["uvx".into(), "skilllite".into(), "mcp".into()],
            "uvx (auto-managed)".into(),
        );
    }

    // Priority 3: skilllite command in PATH
    if which("skilllite") {
        return (
            vec!["skilllite".into(), "mcp".into()],
            "skilllite (in PATH)".into(),
        );
    }

    // Priority 4: python3 with skilllite installed
    if which("python3") {
        if let Ok(output) = Command::new("python3")
            .args(["-c", "import skilllite; print('ok')"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("ok") {
                    return (
                        vec![
                            "python3".into(),
                            "-m".into(),
                            "skilllite.mcp.server".into(),
                        ],
                        "python3 (skilllite installed)".into(),
                    );
                }
            }
        }
    }

    // Fallback: use python3 with full module path
    (
        vec![
            "python3".into(),
            "-m".into(),
            "skilllite.mcp.server".into(),
        ],
        "python3 -m (fallback)".into(),
    )
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Generate skills list markdown for documentation.
fn skills_list_md(skills: &[(String, String)]) -> String {
    if skills.is_empty() {
        "- (No pre-defined skills found. Use skilllite_execute_code for code execution.)\n"
            .to_string()
    } else {
        skills
            .iter()
            .map(|(name, desc)| format!("- **{}**: {}\n", name, desc))
            .collect()
    }
}

// ─── Cursor Rules Content ───────────────────────────────────────────────────

fn generate_cursor_rules(skills: &[(String, String)]) -> String {
    let skills_md = skills_list_md(skills);
    format!(
        r#"---
description: SkillLite - Execute code or pre-defined skills in a secure sandbox
globs:
alwaysApply: false
---

## Overview

SkillLite provides a secure sandbox execution environment via MCP. Code runs isolated in a system-level sandbox (macOS Seatbelt / Linux Namespace), preventing malicious code from affecting the host.

## When to use SkillLite vs terminal

| Scenario | Terminal | SkillLite |
|----------|---------|-----------|
| git operations | ✅ | |
| Read/write project files | ✅ | |
| Execute user-provided code | | ✅ |
| Network/API calls | | ✅ |
| Data analysis | | ✅ |
| Run untrusted scripts | | ✅ |
| Execute potentially dangerous commands | | ✅ |

## Available MCP Tools

### 1. skilllite_execute_code
Execute arbitrary code (Python/JavaScript/Bash) in sandbox.

**Params:**
- `language`: "python" | "javascript" | "bash"
- `code`: Code to execute
- `confirmed`: Whether to confirm execution (required for high-risk code)
- `scan_id`: Scan ID (required when confirming)

### 2. skilllite_run_skill
Execute a pre-defined skill.

**Params:**
- `skill_name`: Skill name
- `input`: Skill input params (JSON object)

### 3. skilllite_list_skills
List all available pre-defined skills. No params.

### 4. skilllite_get_skill_info
Get detailed info for a skill, including input schema.

**Params:**
- `skill_name`: Skill name

### 5. skilllite_scan_code
Scan code safety only, no execution. For pre-checking safety.

**Params:**
- `language`: "python" | "javascript" | "bash"
- `code`: Code to scan

## Pre-defined Skills

{skills_md}

## Examples

### Execute Python code
```
skilllite_execute_code(language="python", code="print(sum(range(1, 101)))")
```

### Handle dangerous code
1. Call `skilllite_execute_code` to execute
2. If returns `requires_confirmation=true`, show user the security issues
3. After user confirms, call again with `confirmed=true` and `scan_id`

### Use pre-defined skills
```
skilllite_list_skills()
skilllite_get_skill_info(skill_name="calculator")
skilllite_run_skill(skill_name="calculator", input={{"operation": "add", "a": 5, "b": 3}})
```
"#
    )
}

// ─── OpenCode SKILL.md Content ──────────────────────────────────────────────

fn generate_opencode_skill_md(skills: &[(String, String)]) -> String {
    let skills_md = skills_list_md(skills);
    format!(
        r#"---
name: skilllite
description: Execute code or pre-defined skills in a secure sandbox. Use when running untrusted code, network requests, or data processing.
---

## Overview

SkillLite provides a secure sandbox execution environment. Code runs isolated in a system-level sandbox (macOS Seatbelt / Linux Namespace), preventing malicious code from affecting the host.

## When to use SkillLite vs bash

| Scenario | bash | SkillLite |
|----------|------|-----------|
| git operations | ✅ | |
| Read project files | ✅ | |
| Execute user-provided code | | ✅ |
| Network/API calls | | ✅ |
| Data analysis | | ✅ |
| Run untrusted scripts | | ✅ |
| Execute potentially dangerous commands | | ✅ |

## Available Tools

### 1. skilllite_execute_code
Execute arbitrary code (Python/JavaScript/Bash) in sandbox.

### 2. skilllite_run_skill
Execute a pre-defined skill.

### 3. skilllite_list_skills
List all available pre-defined skills. No params.

### 4. skilllite_get_skill_info
Get detailed info for a skill, including input schema.

### 5. skilllite_scan_code
Scan code safety only, no execution.

## Pre-defined Skills

{skills_md}
"#
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Public command handlers
// ═══════════════════════════════════════════════════════════════════════════

/// `skilllite ide cursor`
pub fn cmd_cursor(
    project_dir: Option<&str>,
    skills_dir: &str,
    global: bool,
    force: bool,
) -> Result<()> {
    let project = project_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mode_label = if global { "Global" } else { "Project" };
    eprintln!(
        "🚀 Initializing SkillLite integration for Cursor IDE ({})...",
        mode_label
    );
    if !global {
        eprintln!("   Project directory: {}", project.display());
    }
    eprintln!();

    // Detect best MCP command
    let (command, command_desc) = detect_best_mcp_command();
    eprintln!("✓ MCP command: {}", command_desc);
    eprintln!("   → {}", command.join(" "));

    // Determine config paths
    let (cursor_dir, mcp_config_path, skills_dir_for_config) = if global {
        let global_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".cursor");
        let config_path = global_dir.join("mcp.json");
        let sd_clean = skills_dir.strip_prefix("./").unwrap_or(skills_dir);
        let abs_skills = project.join(sd_clean);
        (global_dir, config_path, abs_skills.to_string_lossy().to_string())
    } else {
        let cursor = project.join(".cursor");
        let config_path = cursor.join("mcp.json");
        (cursor, config_path, skills_dir.to_string())
    };

    fs::create_dir_all(&cursor_dir)
        .context("Failed to create .cursor directory")?;

    // Generate MCP config
    let cmd_executable = &command[0];
    let cmd_args: Vec<&str> = command[1..].iter().map(|s| s.as_str()).collect();

    let mcp_entry = json!({
        "command": cmd_executable,
        "args": cmd_args,
        "env": {
            "SKILLBOX_SANDBOX_LEVEL": "3",
            "SKILLLITE_SKILLS_DIR": skills_dir_for_config
        }
    });

    let config = if mcp_config_path.exists() && !force {
        // Merge with existing config
        match fs::read_to_string(&mcp_config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(mut existing) => {
                if !existing.get("mcpServers").is_some() {
                    existing["mcpServers"] = json!({});
                }
                existing["mcpServers"]["skilllite"] = mcp_entry;
                eprintln!("✓ Updated existing {}", mcp_config_path.display());
                existing
            }
            None => {
                eprintln!("⚠ Could not parse existing config, overwriting");
                json!({ "mcpServers": { "skilllite": mcp_entry } })
            }
        }
    } else {
        eprintln!("✓ Created {}", mcp_config_path.display());
        json!({ "mcpServers": { "skilllite": mcp_entry } })
    };

    fs::write(
        &mcp_config_path,
        serde_json::to_string_pretty(&config)?,
    )
    .context("Failed to write mcp.json")?;

    // Get available skills
    let sd_clean = skills_dir.strip_prefix("./").unwrap_or(skills_dir);
    let full_skills_dir = project.join(sd_clean);
    let skills = get_available_skills(&full_skills_dir);
    eprintln!("✓ Found {} skills in {}", skills.len(), skills_dir);

    let mut created_files = vec![mcp_config_path.to_string_lossy().to_string()];

    // Create .cursor/rules/skilllite.mdc (project-level only)
    if !global {
        let rules_dir = cursor_dir.join("rules");
        fs::create_dir_all(&rules_dir).context("Failed to create rules directory")?;

        let rules_path = rules_dir.join("skilllite.mdc");
        let rules_content = generate_cursor_rules(&skills);
        fs::write(&rules_path, rules_content).context("Failed to write rules file")?;
        eprintln!("✓ Created .cursor/rules/skilllite.mdc");
        created_files.push(rules_path.to_string_lossy().to_string());
    }

    // Summary
    eprintln!();
    eprintln!("{}", "=".repeat(50));
    eprintln!(
        "🎉 SkillLite integration for Cursor initialized! ({})",
        mode_label
    );
    eprintln!();
    eprintln!("Created files:");
    for f in &created_files {
        eprintln!("  • {}", f);
    }
    eprintln!();
    eprintln!("Available MCP tools in Cursor:");
    eprintln!("  • skilllite_execute_code - Execute code in sandbox");
    eprintln!("  • skilllite_run_skill    - Run pre-defined skills");
    eprintln!("  • skilllite_list_skills  - List available skills");
    eprintln!("  • skilllite_get_skill_info - Get skill details");
    eprintln!("  • skilllite_scan_code    - Scan code for security issues");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  1. Reload Cursor window (Cmd+Shift+P → Reload Window)");
    eprintln!("  2. Check MCP status in Cursor Settings → MCP");
    eprintln!("  3. Start chatting with Cursor Agent to use SkillLite tools");
    eprintln!("{}", "=".repeat(50));

    Ok(())
}

/// `skilllite ide opencode`
pub fn cmd_opencode(
    project_dir: Option<&str>,
    skills_dir: &str,
    force: bool,
) -> Result<()> {
    let project = project_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    eprintln!("🚀 Initializing SkillLite integration for OpenCode...");
    eprintln!("   Project directory: {}", project.display());
    eprintln!();

    // Detect best MCP command
    let (command, command_desc) = detect_best_mcp_command();
    eprintln!("✓ MCP command: {}", command_desc);
    eprintln!("   → {}", command.join(" "));

    // Create opencode.json
    let opencode_config_path = project.join("opencode.json");

    let mcp_entry = json!({
        "type": "local",
        "command": command,
        "environment": {
            "SKILLBOX_SANDBOX_LEVEL": "3",
            "SKILLLITE_SKILLS_DIR": skills_dir
        },
        "enabled": true
    });

    let config = if opencode_config_path.exists() && !force {
        match fs::read_to_string(&opencode_config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(mut existing) => {
                if !existing.get("mcp").is_some() {
                    existing["mcp"] = json!({});
                }
                existing["mcp"]["skilllite"] = mcp_entry;
                if !existing.get("$schema").is_some() {
                    existing["$schema"] = json!("https://opencode.ai/config.json");
                }
                eprintln!("✓ Updated existing opencode.json");
                existing
            }
            None => {
                eprintln!("⚠ Could not parse existing opencode.json, overwriting");
                json!({
                    "$schema": "https://opencode.ai/config.json",
                    "mcp": { "skilllite": mcp_entry }
                })
            }
        }
    } else {
        eprintln!("✓ Created opencode.json");
        json!({
            "$schema": "https://opencode.ai/config.json",
            "mcp": { "skilllite": mcp_entry }
        })
    };

    fs::write(
        &opencode_config_path,
        serde_json::to_string_pretty(&config)?,
    )
    .context("Failed to write opencode.json")?;

    // Get available skills
    let sd_clean = skills_dir.strip_prefix("./").unwrap_or(skills_dir);
    let full_skills_dir = project.join(sd_clean);
    let skills = get_available_skills(&full_skills_dir);
    eprintln!("✓ Found {} skills in {}", skills.len(), skills_dir);

    // Create .opencode/skills/skilllite/SKILL.md
    let skill_dir = project.join(".opencode").join("skills").join("skilllite");
    fs::create_dir_all(&skill_dir).context("Failed to create .opencode/skills/skilllite")?;

    let skill_md_path = skill_dir.join("SKILL.md");
    let skill_md_content = generate_opencode_skill_md(&skills);
    fs::write(&skill_md_path, skill_md_content).context("Failed to write SKILL.md")?;
    eprintln!("✓ Created .opencode/skills/skilllite/SKILL.md");

    // Summary
    eprintln!();
    eprintln!("{}", "=".repeat(50));
    eprintln!("🎉 SkillLite integration initialized successfully!");
    eprintln!();
    eprintln!("Created files:");
    eprintln!("  • opencode.json");
    eprintln!("  • .opencode/skills/skilllite/SKILL.md");
    eprintln!();
    eprintln!("Available MCP tools in OpenCode:");
    eprintln!("  • skilllite_execute_code - Execute code in sandbox");
    eprintln!("  • skilllite_run_skill    - Run pre-defined skills");
    eprintln!("  • skilllite_list_skills  - List available skills");
    eprintln!("  • skilllite_get_skill_info - Get skill details");
    eprintln!("  • skilllite_scan_code    - Scan code for security issues");
    eprintln!();
    eprintln!("Start OpenCode with: opencode");
    eprintln!("{}", "=".repeat(50));

    Ok(())
}
