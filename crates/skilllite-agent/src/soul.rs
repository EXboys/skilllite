//! Agent constitution: Law + Beliefs + Soul.
//!
//! Single module for agent identity and constraints. Per ROADMAP: Soul = Law + Beliefs + MinimalCapabilities.
//!
//! ## Law (不可变)
//! Built-in immutable constraints, always applied. Cannot be overridden.
//!
//! ## Beliefs (可进化)
//! Derived from existing evolution outputs — no separate file.
//! decision_tendency ← rules.json, success_patterns ← examples.json.
//!
//! ## Soul (SOUL.md)
//! User-provided identity document. Read-only at runtime.
//! Storage resolution (first found wins):
//!   1. Explicit `--soul <path>` CLI flag
//!   2. `.skilllite/SOUL.md` (workspace-level)
//!   3. `~/.skilllite/SOUL.md` (global fallback)
//!   If none found, returns `None` — no automatic creation.
//!   Optional first-run guidance: `offer_bootstrap_soul_if_missing()` can prompt to create a minimal template.
//!
//! Format (Markdown with `##` section headings):
//!   ## Identity | ## Core Beliefs | ## Communication Style | ## Scope & Boundaries

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;

/// Minimal SOUL template for optional first-run bootstrap (no preset role).
/// Used by `offer_bootstrap_soul_if_missing` and by tests.
pub const MINIMAL_SOUL_TEMPLATE: &str = r#"# SOUL — Optional identity & constraints

## Identity
(可选 — 由用户或进化定义)

## Core Beliefs
(可选)

## Communication Style
(可选)

## Scope & Boundaries
- WILL NOT: modify SOUL.md; bypass sandbox rules.
"#;

// ─── Law: 内置不可变约束 ─────────────────────────────────────────────────────

/// Built-in immutable constraints. Always applied to every agent.
#[derive(Debug, Clone, Default)]
pub struct Law;

impl Law {
    /// Returns the built-in immutable constraints as a system prompt block.
    pub fn to_system_prompt_block(&self) -> String {
        const LAW_RULES: &str = r#"╔═══════════════════════════════════╗
║  LAW — Immutable Constraints       ║
║  These rules cannot be overridden. ║
╚═══════════════════════════════════╝

### Law (MANDATORY)

- **Do not harm humans.** Never suggest or execute actions that could physically, psychologically, or financially harm users or third parties.
- **Do not leak privacy.** Never store, transmit, or expose user data, credentials, or sensitive information outside the intended scope. Respect local-first: data stays on the user's machine unless explicitly authorized.
- **Do not self-destruct.** Never suggest or execute actions that would permanently destroy the agent's ability to operate, corrupt the workspace irreversibly, or remove critical system components without explicit user confirmation.
"#;
        format!("\n\n{}", LAW_RULES)
    }
}

// ─── Beliefs: 可进化行为模式（派生自现有进化产出）────────────────────────────────

/// Beliefs 不新增文件，从 rules.json + examples.json 派生。
/// 对应关系：decision_tendency ← rules，success_patterns ← examples，knowledge.md 倾向/模式由 memory 检索注入。
const BELIEFS_RULES_TOP: usize = 5;
const BELIEFS_EXAMPLES_TOP: usize = 3;

/// Build Beliefs prompt block from existing evolution outputs.
/// No beliefs.json — derives from prompts/rules.json and prompts/examples.json.
/// Only evolved rules (mutable or origin != "seed") are shown; seed rules are excluded.
pub fn build_beliefs_block(chat_root: &Path) -> String {
    let rules = skilllite_evolution::seed::load_rules(chat_root);
    let decision_tendency: String = rules
        .iter()
        .filter(|r| r.mutable || r.origin != "seed")
        .take(BELIEFS_RULES_TOP)
        .filter(|r| !r.instruction.is_empty())
        .map(|r| format!("- {}", r.instruction.trim().lines().next().unwrap_or("").trim()))
        .filter(|s| !s.eq("- "))
        .collect::<Vec<_>>()
        .join("\n");

    let success_patterns = load_examples_key_insights(chat_root);

    if decision_tendency.is_empty() && success_patterns.is_empty() {
        return String::new();
    }

    let mut parts = vec![
        "\n\n╔═══════════════════════════════════╗".to_string(),
        "║  Beliefs — From evolved rules/examples ║".to_string(),
        "╚═══════════════════════════════════╝".to_string(),
    ];
    if !decision_tendency.is_empty() {
        parts.push(format!("\n### Decision Tendency (from rules)\n{}", decision_tendency));
    }
    if !success_patterns.is_empty() {
        parts.push(format!("\n### Success Patterns (from examples)\n{}", success_patterns));
    }
    parts.push("═══════════════════════════════════".to_string());
    parts.join("\n")
}

fn load_examples_key_insights(chat_root: &Path) -> String {
    let path = chat_root.join("prompts").join("examples.json");
    if !path.exists() {
        return String::new();
    }
    let content = match skilllite_fs::read_file(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    #[derive(serde::Deserialize)]
    struct Ex { key_insight: Option<String> }
    let arr: Vec<Ex> = match serde_json::from_str(&content) {
        Ok(a) => a,
        Err(_) => return String::new(),
    };
    arr.iter()
        .take(BELIEFS_EXAMPLES_TOP)
        .filter_map(|e| e.key_insight.as_deref())
        .filter(|s| !s.is_empty())
        .map(|s| format!("- {}", s.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Soul: SOUL.md 解析与加载 ────────────────────────────────────────────────

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
        let content = skilllite_fs::read_file(path)
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
        }

        None
    }

    /// If no SOUL exists in the resolution chain and stdin is a TTY, prompt the user to create a minimal
    /// template at `workspace/.skilllite/SOUL.md`. When the user confirms (y/Y), write `MINIMAL_SOUL_TEMPLATE`
    /// and return `true`; otherwise return `false`. Does nothing when `explicit_path` is `Some` (user already
    /// chose a path) or when not interactive (no TTY).
    pub fn offer_bootstrap_soul_if_missing(workspace: &str, explicit_path: Option<&str>) -> bool {
        if explicit_path.is_some() {
            return false;
        }
        if Self::auto_load(None, workspace).is_some() {
            return false;
        }
        if !io::stdin().is_terminal() {
            return false;
        }
        let path = Path::new(workspace).join(".skilllite").join("SOUL.md");
        eprint!(
            "No SOUL.md found. Create minimal template at {}? [y/N] ",
            path.display()
        );
        let _ = io::stderr().flush();
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).is_err() {
            return false;
        }
        let trimmed = line.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            return false;
        }
        if let Some(parent) = path.parent() {
            let _ = skilllite_fs::create_dir_all(parent);
        }
        skilllite_fs::write_file(&path, MINIMAL_SOUL_TEMPLATE).is_ok()
    }

    /// Render the SOUL Scope & Boundaries as a planning constraint block (A8).
    ///
    /// Injected into the planning prompt so the LLM respects "in scope" / "out of scope"
    /// when generating task lists. Planning must NOT create tasks that violate these rules.
    pub fn to_planning_scope_block(&self) -> Option<String> {
        if self.scope_and_boundaries.is_empty() {
            return None;
        }
        Some(format!(
            "\n## SOUL Scope & Boundaries (MANDATORY for planning)\n\
             When generating the task list, you MUST respect these boundaries.\n\
             ONLY plan tasks that fall within scope. Do NOT plan any task that violates \"Will Not Do\" / out-of-scope rules.\n\n\
             {}\n",
            self.scope_and_boundaries.trim()
        ))
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

    #[test]
    fn test_parse_all_sections() {
        let soul = Soul::parse(MINIMAL_SOUL_TEMPLATE, "test/SOUL.md");
        assert!(soul.identity.contains("可选"));
        assert!(soul.core_beliefs.contains("可选"));
        assert!(soul.communication_style.contains("可选"));
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
        let soul = Soul::parse(MINIMAL_SOUL_TEMPLATE, "SOUL.md");
        let block = soul.to_system_prompt_block();
        assert!(block.contains("SOUL"));
        assert!(block.contains("Identity"));
        assert!(block.contains("Scope & Boundaries"));
        assert!(block.contains("read-only"));
    }

    #[test]
    fn test_to_planning_scope_block() {
        let soul = Soul::parse(MINIMAL_SOUL_TEMPLATE, "SOUL.md");
        let block = soul.to_planning_scope_block().unwrap();
        assert!(block.contains("SOUL Scope & Boundaries"));
        assert!(block.contains("MANDATORY"));
        assert!(block.contains("WILL NOT"));
        assert!(block.contains("modify SOUL.md"));

        let empty_soul = Soul::parse("", "empty.md");
        assert!(empty_soul.to_planning_scope_block().is_none());
    }

    #[test]
    fn test_sample_soul_parses_all_sections() {
        let soul = Soul::parse(MINIMAL_SOUL_TEMPLATE, "test/SOUL.md");
        assert!(!soul.identity.is_empty(), "sample has identity section");
        assert!(!soul.core_beliefs.is_empty(), "sample has core_beliefs section");
        assert!(!soul.communication_style.is_empty(), "sample has communication_style section");
        assert!(!soul.scope_and_boundaries.is_empty(), "sample has scope_and_boundaries");
    }

    #[test]
    fn test_law_prompt_contains_mandatory_rules() {
        let law = Law::default();
        let block = law.to_system_prompt_block();
        assert!(block.contains("LAW"));
        assert!(block.contains("Do not harm humans"));
        assert!(block.contains("Do not leak privacy"));
        assert!(block.contains("Do not self-destruct"));
    }

    #[test]
    fn test_build_beliefs_block_empty_when_rules_and_examples_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let prompts_dir = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts_dir).unwrap();
        std::fs::write(prompts_dir.join("rules.json"), "[]").unwrap();
        let block = build_beliefs_block(tmp.path());
        assert!(block.is_empty());
    }

    #[test]
    fn test_build_beliefs_block_from_rules() {
        let tmp = tempfile::tempdir().unwrap();
        let prompts_dir = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts_dir).unwrap();
        let rules = r#"[{"id":"r1","instruction":"Use read_file before edit.","mutable":true}]"#;
        std::fs::write(prompts_dir.join("rules.json"), rules).unwrap();
        let block = build_beliefs_block(tmp.path());
        assert!(block.contains("Beliefs"));
        assert!(block.contains("Decision Tendency"));
        assert!(block.contains("Use read_file before edit"));
    }

    #[test]
    fn test_build_beliefs_block_from_examples() {
        let tmp = tempfile::tempdir().unwrap();
        let prompts_dir = tmp.path().join("prompts");
        std::fs::create_dir_all(&prompts_dir).unwrap();
        let examples = r#"[{"id":"e1","task_pattern":"x","plan_template":"y","key_insight":"Read then edit."}]"#;
        std::fs::write(prompts_dir.join("examples.json"), examples).unwrap();
        let block = build_beliefs_block(tmp.path());
        assert!(block.contains("Beliefs"));
        assert!(block.contains("Success Patterns"));
        assert!(block.contains("Read then edit"));
    }
}

