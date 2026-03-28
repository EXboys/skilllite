//! Planning rules for task generation.
//!
//! EVO-2: Rules are loaded from `~/.skilllite/chat/prompts/rules.json` at runtime.
//! Compiled-in seed data provides the fallback when no external file exists.
//! Edit the external JSON file to add or modify rules without recompiling.
//!
//! EVO-3 fix: Workspace rules (per-project, from `init --use-llm`) are now **merged**
//! with global rules (seed + evolved) instead of replacing them. This ensures evolved
//! rules are never silently discarded when a workspace file exists.

use std::path::Path;

use super::types::PlanningRule;
use skilllite_evolution::seed;

/// Load planning rules.
///
/// Resolution:
/// 1. Global `~/.skilllite/chat/prompts/rules.json` (seed + evolved) — always loaded
/// 2. Workspace `.skilllite/planning_rules.json` (per-project skill rules) — **merged on top**
/// 3. Compiled-in seed data (fallback when no global file)
///
/// Merge semantics:
/// - Workspace rule with same ID as an immutable (`mutable=false`) global rule → skipped
/// - Workspace rule with same ID as a mutable global rule → overrides it
/// - Workspace rule with a new ID → appended
pub fn load_rules(workspace: Option<&Path>, chat_root: Option<&Path>) -> Vec<PlanningRule> {
    // Base: global rules (seed + evolved)
    let mut rules = if let Some(root) = chat_root {
        seed::load_rules(root)
    } else {
        seed::load_rules(Path::new("/nonexistent"))
    };

    // Merge workspace-specific rules (per-project skill rules from `init --use-llm`)
    if let Some(ws) = workspace {
        let path = ws.join(".skilllite").join("planning_rules.json");
        if path.exists() {
            if let Ok(content) = skilllite_fs::read_file(&path) {
                if let Ok(ws_rules) = serde_json::from_str::<Vec<PlanningRule>>(&content) {
                    let ws_count = ws_rules.len();
                    merge_workspace_rules(&mut rules, ws_rules);
                    tracing::debug!(
                        "Merged {} workspace rules from {}",
                        ws_count,
                        path.display()
                    );
                }
            }
        }
    }

    rules
}

/// Merge workspace rules into the base rule set.
///
/// - Same ID + base is immutable → skip (seed rules cannot be overridden)
/// - Same ID + base is mutable → override (workspace takes priority)
/// - New ID → append
fn merge_workspace_rules(base: &mut Vec<PlanningRule>, workspace: Vec<PlanningRule>) {
    for ws_rule in workspace {
        if let Some(pos) = base.iter().position(|r| r.id == ws_rule.id) {
            if base[pos].mutable {
                base[pos] = ws_rule;
            }
            // Immutable (seed) rules are never overridden — silently skip
        } else {
            base.push(ws_rule);
        }
    }
}

/// Load full examples text from disk or compiled-in seed.
pub fn load_full_examples(chat_root: Option<&Path>) -> String {
    if let Some(root) = chat_root {
        return seed::load_examples(root);
    }
    include_str!("seed/examples.seed.md").to_string()
}

/// Compact examples section: core examples + up to 3 matched by user message keywords.
/// Only references builtin tools — skill-specific examples come from evolution.
pub fn compact_examples_section(user_message: &str) -> String {
    let msg_lower = user_message.to_lowercase();
    let mut lines = vec![
        "Example 1 - Simple (no tools): \"Write a poem\", \"Translate X\", \"Explain this code\" → []"
            .to_string(),
        "Example 2 - File output: \"写一篇文章，保存到output\" → [{\"id\":1,\"description\":\"Generate content and save with write_output\",\"tool_hint\":\"file_write\",\"completed\":false}]"
            .to_string(),
    ];
    let candidates: Vec<(&str, &str, &str)> = vec![
        (
            "分析",
            "稳定性",
            "分析稳定性/项目: chat_history (ONLY when analyzing chat/project)",
        ),
        ("历史", "记录", "历史记录: chat_history + analysis."),
        (
            "输出到",
            "保存到",
            "输出到output: write_output, file_write.",
        ),
        ("官网", "网站", "官网/网站: file_write + preview, 2 tasks."),
        (
            "refactor",
            "panic",
            "编码refactor: file_read→file_edit→command.",
        ),
        ("整理", "项目", "模糊请求: file_list探索→analysis总结/确认."),
    ];
    let mut added = 0;
    for (k1, k2, text) in candidates {
        if added >= 3 {
            break;
        }
        let matches = user_message.contains(k1)
            || msg_lower.contains(&k1.to_lowercase())
            || (!k2.is_empty()
                && (user_message.contains(k2) || msg_lower.contains(&k2.to_lowercase())));
        if matches {
            lines.push(format!("Example - {}: {}", k1, text));
            added += 1;
        }
    }
    lines.join("\n")
}
