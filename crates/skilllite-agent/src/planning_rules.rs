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

use super::evolution::seed;
use super::types::PlanningRule;

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
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(ws_rules) = serde_json::from_str::<Vec<PlanningRule>>(&content) {
                    let ws_count = ws_rules.len();
                    merge_workspace_rules(&mut rules, ws_rules);
                    tracing::debug!("Merged {} workspace rules from {}", ws_count, path.display());
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
/// Runtime logic preserved (cannot be fully externalized as it depends on user input).
pub fn compact_examples_section(user_message: &str) -> String {
    let msg_lower = user_message.to_lowercase();
    let mut lines = vec![
        "Example 1 - Simple (no tools): \"Write a poem\", \"Translate X\", \"Explain this code\" → []".to_string(),
        "Example 2 - Tools: \"Calculate 123*456\" → [{\"id\":1,\"description\":\"Use calculator\",\"tool_hint\":\"calculator\",\"completed\":false}]".to_string(),
    ];
    let is_city_or_place = user_message.contains("城市")
        || user_message.contains("地方")
        || user_message.contains("对比")
        || user_message.contains("优劣势")
        || user_message.contains("全方位")
        || user_message.contains("两地")
        || msg_lower.contains("city")
        || msg_lower.contains("place");
    let candidates: Vec<(&str, &str, &str)> = vec![
        ("介绍", "景点", "介绍+地点/景点/路线: agent-browser or http-request for fresh info. NOT []."),
        ("城市", "全方位", "城市/地方/全方位分析: http-request for fresh data. NOT chat_history."),
        ("对比", "优劣势", "对比/优劣势: http-request for fresh data. NOT chat_history."),
        ("分析", "稳定性", "分析稳定性/项目: chat_history (ONLY when analyzing chat/project, NOT places)"),
        ("历史", "记录", "历史记录: chat_history + analysis."),
        ("输出到", "保存到", "输出到output: write_output, file_operation."),
        ("继续", "", "继续: use context to infer task, often http-request."),
        ("天气", "气象", "天气: weather skill."),
        ("官网", "网站", "官网/网站: write_output + preview_server, 2 tasks."),
        ("refactor", "panic", "编码refactor: grep定位→search_replace→run_command测试."),
        ("整理", "项目", "模糊请求: list_directory探索→分析执行."),
    ];
    let mut added = 0;
    for (k1, k2, text) in candidates {
        if added >= 3 {
            break;
        }
        let matches = user_message.contains(k1)
            || msg_lower.contains(&k1.to_lowercase())
            || (!k2.is_empty() && (user_message.contains(k2) || msg_lower.contains(&k2.to_lowercase())));
        let skip = matches
            && k1 == "分析"
            && is_city_or_place;
        if matches && !skip {
            lines.push(format!("Example - {}: {}", k1, text));
            added += 1;
        }
    }
    lines.join("\n")
}
