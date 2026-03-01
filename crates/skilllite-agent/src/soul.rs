//! SOUL.md — Agent identity document.
//!
//! Runtime completely read-only: loaded once at startup, never written by the agent.
//! The agent's "constitutional document": identity, beliefs, communication style, boundaries.
//!
//! Storage resolution (first found wins):
//!   1. Explicit `--soul <path>` CLI flag
//!   2. `.skilllite/SOUL.md` (workspace-level, per-project)
//!   3. `~/.skilllite/SOUL.md` (global fallback)
//!   4. Bootstrap: if none found, write seed template to `~/.skilllite/SOUL.md` once
//!
//! Format (Markdown with `##` section headings):
//!   ## Identity
//!   ## Core Beliefs
//!   ## Communication Style
//!   ## Scope & Boundaries

use std::path::{Path, PathBuf};

use anyhow::Result;

/// Compiled-in seed template for SOUL.md.
///
/// Written to `~/.skilllite/SOUL.md` on first run when no SOUL.md is found anywhere.
/// Never overwritten after that — the user owns this file.
pub const SEED_SOUL: &str = r#"# SOUL.md — Agent Identity Document
#
# This file defines who this agent is and what it will/won't do.
# It is loaded at startup and is READ-ONLY at runtime — the agent cannot modify it.
# Edit this file freely. Changes take effect on next agent startup.

## Identity

You are a focused, reliable AI coding assistant embedded in the SkillLite workspace.
Your role is to help the developer write, review, debug, and improve code — efficiently and without fluff.
You operate locally, respect the user's privacy, and stay within the scope of tasks you are given.

## Core Beliefs

- Correctness comes before speed. A working solution is more valuable than a fast wrong one.
- Security is non-negotiable. Never suggest patterns that expose credentials, bypass sandboxes, or weaken access controls.
- Clarity beats cleverness. Readable, maintainable code is the goal.
- Always verify before acting. When uncertain, ask — don't guess and overwrite.
- Respect the user's existing conventions. Match the code style, naming, and architecture already present in the project.

## Communication Style

- Reply in the same language the user writes in (Chinese or English).
- Be concise. Skip unnecessary preamble — get to the answer.
- Use code blocks for all code snippets, diffs, and file content.
- When explaining, be direct and specific. Avoid vague affirmations like "Great question!".
- For multi-step tasks, show progress clearly so the user knows what has been done and what is next.

## Scope & Boundaries

### Will Do
- Write, edit, refactor, and review code across all files in the workspace
- Run shell commands, tests, and build tools when needed
- Read and summarize documentation, logs, and error output
- Search the codebase and explain how things work
- Help design architecture, data models, and API contracts

### Will Not Do
- Modify this SOUL.md file (it is the agent's constitution — hands off)
- Delete files or directories without explicit user confirmation
- Commit or push code to version control without being asked
- Access URLs or external services outside the scope of the current task
- Store or transmit any user data externally
"#;

/// Parsed representation of a SOUL.md document.
///
/// Runtime completely read-only — user-provided identity document.
/// The agent cannot write or modify this struct after loading.
#[derive(Debug, Clone)]
pub struct Soul {
    /// `## Identity` — who the agent is: name, role, persona description
    pub identity: String,
    /// `## Core Beliefs` — non-negotiable values (OpenClaw: "Core Truths")
    pub core_beliefs: String,
    /// `## Communication Style` — tone, language preferences, reply style
    pub communication_style: String,
    /// `## Scope & Boundaries` — what the agent will and will not do
    pub scope_and_boundaries: String,
    /// Source path (for display/logging only)
    pub source_path: String,
}

impl Soul {
    /// Parse a SOUL.md string into a Soul struct.
    ///
    /// Section detection is case-insensitive. Content before the first `##`
    /// heading is treated as a preamble and ignored.
    pub fn parse(content: &str, source_path: &str) -> Self {
        #[derive(PartialEq)]
        enum Section {
            None,
            Identity,
            CoreBeliefs,
            CommunicationStyle,
            ScopeAndBoundaries,
            Other,
        }

        let mut identity = String::new();
        let mut core_beliefs = String::new();
        let mut communication_style = String::new();
        let mut scope_and_boundaries = String::new();
        let mut current = Section::None;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("## ") {
                let heading = trimmed[3..].trim().to_lowercase();
                current = match heading.as_str() {
                    "identity" => Section::Identity,
                    "core beliefs" | "core_beliefs" | "corebeliefs" => Section::CoreBeliefs,
                    "communication style" | "communication_style" => Section::CommunicationStyle,
                    "scope & boundaries" | "scope and boundaries"
                    | "scope_and_boundaries" | "scope" => Section::ScopeAndBoundaries,
                    _ => Section::Other,
                };
                continue;
            }

            let target = match current {
                Section::Identity => Some(&mut identity),
                Section::CoreBeliefs => Some(&mut core_beliefs),
                Section::CommunicationStyle => Some(&mut communication_style),
                Section::ScopeAndBoundaries => Some(&mut scope_and_boundaries),
                _ => None,
            };
            if let Some(buf) = target {
                buf.push_str(line);
                buf.push('\n');
            }
        }

        Soul {
            identity: identity.trim().to_string(),
            core_beliefs: core_beliefs.trim().to_string(),
            communication_style: communication_style.trim().to_string(),
            scope_and_boundaries: scope_and_boundaries.trim().to_string(),
            source_path: source_path.to_string(),
        }
    }

    /// Load a SOUL.md file from disk.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read SOUL.md at {}: {}", path.display(), e))?;
        Ok(Self::parse(&content, &path.to_string_lossy()))
    }

    /// Auto-discover and load SOUL.md from the resolution chain.
    ///
    /// Returns `None` if no SOUL.md is found anywhere in the chain.
    pub fn auto_load(explicit_path: Option<&str>, workspace: &str) -> Option<Self> {
        // 1. Explicit --soul flag
        if let Some(p) = explicit_path {
            let path = PathBuf::from(p);
            match Self::load(&path) {
                Ok(soul) => {
                    tracing::info!("SOUL loaded from explicit path: {}", p);
                    return Some(soul);
                }
                Err(e) => {
                    tracing::warn!("Failed to load SOUL from explicit path {}: {}", p, e);
                    return None;
                }
            }
        }

        // 2. Workspace .skilllite/SOUL.md
        let ws_soul = Path::new(workspace).join(".skilllite").join("SOUL.md");
        if ws_soul.exists() {
            match Self::load(&ws_soul) {
                Ok(soul) => {
                    tracing::info!("SOUL loaded from workspace: {}", ws_soul.display());
                    return Some(soul);
                }
                Err(e) => {
                    tracing::warn!("Failed to load workspace SOUL: {}", e);
                }
            }
        }

        // 3. Global ~/.skilllite/SOUL.md
        if let Some(home) = dirs::home_dir() {
            let global_soul = home.join(".skilllite").join("SOUL.md");
            if global_soul.exists() {
                match Self::load(&global_soul) {
                    Ok(soul) => {
                        tracing::info!("SOUL loaded from global: {}", global_soul.display());
                        return Some(soul);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load global SOUL: {}", e);
                    }
                }
            }

            // 4. Bootstrap: no SOUL.md found anywhere — write seed template once
            if let Some(soul) = Self::bootstrap_global_soul(&global_soul) {
                return Some(soul);
            }
        }

        None
    }

    /// Write the seed SOUL.md template to `~/.skilllite/SOUL.md` on first run.
    ///
    /// Only called when no SOUL.md exists anywhere in the resolution chain.
    /// Never overwrites an existing file — user edits are always preserved.
    fn bootstrap_global_soul(path: &Path) -> Option<Self> {
        if path.exists() {
            return None; // Safety guard: never overwrite
        }
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                tracing::warn!("SOUL bootstrap: failed to create dir {}", parent.display());
                return None;
            }
        }
        match std::fs::write(path, SEED_SOUL) {
            Ok(_) => {
                tracing::info!("SOUL bootstrapped to {}", path.display());
                Self::load(path).ok()
            }
            Err(e) => {
                tracing::warn!("SOUL bootstrap failed: {}", e);
                None
            }
        }
    }

    /// Render the SOUL as a system prompt injection block.
    ///
    /// Only non-empty sections are included to keep the prompt lean.
    pub fn to_system_prompt_block(&self) -> String {
        let mut parts = vec![
            "\n\n╔═══════════════════════════════════╗".to_string(),
            format!("║  SOUL  (source: {})", self.source_path),
            "║  This document defines your identity and non-negotiable constraints.".to_string(),
            "║  It is read-only — you must never modify or override any of its rules.".to_string(),
            "╚═══════════════════════════════════╝".to_string(),
        ];

        if !self.identity.is_empty() {
            parts.push(format!("\n### Identity\n{}", self.identity));
        }
        if !self.core_beliefs.is_empty() {
            parts.push(format!("\n### Core Beliefs\n{}", self.core_beliefs));
        }
        if !self.communication_style.is_empty() {
            parts.push(format!("\n### Communication Style\n{}", self.communication_style));
        }
        if !self.scope_and_boundaries.is_empty() {
            parts.push(format!("\n### Scope & Boundaries\n{}", self.scope_and_boundaries));
        }

        parts.push("═══════════════════════════════════".to_string());
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SOUL: &str = r#"# My Agent SOUL

## Identity
I am SkillBot, a focused Rust coding assistant.
I specialize in SkillLite plugin development.

## Core Beliefs
- Code correctness is non-negotiable.
- Security > performance > convenience.
- Never guess; always verify with tests.

## Communication Style
- Reply in the same language as the user.
- Be concise; avoid unnecessary filler words.
- Use code blocks for all code snippets.

## Scope & Boundaries
- WILL: help with Rust, SKILL.md authoring, tool design.
- WILL NOT: write exploits, bypass sandbox rules, or modify SOUL.md.
"#;

    #[test]
    fn test_parse_all_sections() {
        let soul = Soul::parse(SAMPLE_SOUL, "test/SOUL.md");
        assert!(soul.identity.contains("SkillBot"));
        assert!(soul.core_beliefs.contains("correctness"));
        assert!(soul.communication_style.contains("concise"));
        assert!(soul.scope_and_boundaries.contains("WILL NOT"));
        assert_eq!(soul.source_path, "test/SOUL.md");
    }

    #[test]
    fn test_parse_empty_content() {
        let soul = Soul::parse("", "empty.md");
        assert!(soul.identity.is_empty());
        assert!(soul.core_beliefs.is_empty());
    }

    #[test]
    fn test_to_system_prompt_block_contains_sections() {
        let soul = Soul::parse(SAMPLE_SOUL, "SOUL.md");
        let block = soul.to_system_prompt_block();
        assert!(block.contains("SOUL"));
        assert!(block.contains("Identity"));
        assert!(block.contains("Core Beliefs"));
        assert!(block.contains("read-only"));
    }

    #[test]
    fn test_bootstrap_writes_seed_template() {
        let tmp = tempfile::tempdir().unwrap();
        let soul_path = tmp.path().join("SOUL.md");

        // File must not exist before bootstrap
        assert!(!soul_path.exists());

        let soul = Soul::bootstrap_global_soul(&soul_path);
        assert!(soul.is_some(), "bootstrap should return a Soul");
        assert!(soul_path.exists(), "bootstrap should write the file");

        let content = std::fs::read_to_string(&soul_path).unwrap();
        assert!(content.contains("## Identity"));
        assert!(content.contains("## Core Beliefs"));
        assert!(content.contains("## Communication Style"));
        assert!(content.contains("## Scope & Boundaries"));
    }

    #[test]
    fn test_bootstrap_never_overwrites_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let soul_path = tmp.path().join("SOUL.md");
        std::fs::write(&soul_path, "## Identity\nCustom content").unwrap();

        let soul = Soul::bootstrap_global_soul(&soul_path);
        // Should return None — never overwrites
        assert!(soul.is_none());
        // File content must be unchanged
        let content = std::fs::read_to_string(&soul_path).unwrap();
        assert_eq!(content, "## Identity\nCustom content");
    }

    #[test]
    fn test_seed_soul_parses_all_sections() {
        let soul = Soul::parse(SEED_SOUL, "seed");
        assert!(!soul.identity.is_empty(), "seed identity should not be empty");
        assert!(!soul.core_beliefs.is_empty(), "seed core_beliefs should not be empty");
        assert!(!soul.communication_style.is_empty(), "seed communication_style should not be empty");
        assert!(!soul.scope_and_boundaries.is_empty(), "seed scope_and_boundaries should not be empty");
    }
}

