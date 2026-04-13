//! Prompt learner: extract rules and examples from execution feedback (EVO-3).

use std::path::Path;

use crate::error::bail;
use crate::Result;
use rusqlite::{params, Connection};
use tokio::task::block_in_place;

use skilllite_core::config::env_keys::evolution as evo_keys;
use skilllite_core::planning::PlanningRule;

use crate::feedback::compute_effectiveness;
use crate::{
    gatekeeper_l1_path, gatekeeper_l2_size, gatekeeper_l3_content, EvolutionLlm, EvolutionMessage,
};
use skilllite_fs::atomic_write;

const RULE_EXTRACTION_PROMPT: &str = include_str!("seed/evolution_prompts/rule_extraction.seed.md");
const EXAMPLE_GENERATION_PROMPT: &str =
    include_str!("seed/evolution_prompts/example_generation.seed.md");

/// Effectiveness below which a rule is retired (aligned with skill retire threshold).
const RETIRE_EFFECTIVENESS_THRESHOLD: f32 = 0.3;
/// Minimum trigger count before a rule is eligible for retirement (need enough data).
const RETIRE_MIN_TRIGGER_COUNT: i64 = 5;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningExample {
    pub id: String,
    pub task_pattern: String,
    pub plan_template: String,
    pub key_insight: String,
    #[serde(default = "default_evolved_origin")]
    pub origin: String,
}

fn default_evolved_origin() -> String {
    "evolved".to_string()
}

fn prompt_example_min_tools() -> i64 {
    std::env::var(evo_keys::SKILLLITE_EVO_PROMPT_EXAMPLE_MIN_TOOLS)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .clamp(1, 20)
}

fn prompt_rule_summary_limit() -> i64 {
    std::env::var(evo_keys::SKILLLITE_EVO_PROMPT_RULE_SUMMARY_LIMIT)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
        .clamp(1, 50)
}

pub async fn evolve_prompts<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
) -> Result<Vec<(String, String)>> {
    let mut changes = Vec::new();

    // Batch all DB operations in one block_in_place to reduce connection opens.
    let (retired, extract_data, example_data) = block_in_place(|| {
        let conn = crate::feedback::open_evolution_db(chat_root)?;
        let retired = retire_low_effectiveness_rules_with_conn(chat_root, txn_id, &conn)?;
        let rule_limit = prompt_rule_summary_limit();
        let successful = query_decisions_summary(&conn, true, rule_limit)?;
        let failed = query_decisions_summary(&conn, false, rule_limit)?;
        let min_tools = prompt_example_min_tools();
        let example_sql = format!(
            "SELECT task_description, tools_detail, elapsed_ms
             FROM decisions
             WHERE evolved = 0 AND task_completed = 1 AND replans = 0
                   AND failed_tools = 0 AND total_tools >= {}
             ORDER BY total_tools DESC LIMIT 1",
            min_tools
        );
        let example_candidate = conn.query_row(&example_sql, [], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        });
        let example_data = example_candidate.ok();
        Ok::<_, anyhow::Error>((retired, (successful, failed), example_data))
    })?;

    changes.extend(retired);

    let rule_changes = extract_rules_from_data(chat_root, extract_data, llm, model).await?;
    changes.extend(rule_changes);

    let example_changes = generate_examples_from_data(chat_root, example_data, llm, model).await?;
    changes.extend(example_changes);

    let new_rules = changes.iter().filter(|(t, _)| t == "rule_added").count();
    let new_examples = changes.iter().filter(|(t, _)| t == "example_added").count();
    if !gatekeeper_l2_size(new_rules, new_examples, 0) {
        tracing::warn!(
            "Gatekeeper L2: evolution produced too many changes (rules={}, examples={}), truncating",
            new_rules, new_examples
        );
        changes.truncate(5 + 3);
    }

    Ok(changes)
}

async fn extract_rules_from_data<L: EvolutionLlm>(
    chat_root: &Path,
    (successful, failed): (String, String),
    llm: &L,
    model: &str,
) -> Result<Vec<(String, String)>> {
    if successful.is_empty() && failed.is_empty() {
        return Ok(Vec::new());
    }

    let existing_rules = crate::seed::load_rules(chat_root);
    let existing_summary = existing_rules
        .iter()
        .map(|r| format!("- {}: {}", r.id, r.instruction))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = RULE_EXTRACTION_PROMPT
        .replace("{{existing_rules_summary}}", &existing_summary)
        .replace("{{successful_decisions}}", &successful)
        .replace("{{failed_decisions}}", &failed);

    let messages = vec![EvolutionMessage::user(&prompt)];
    let content = llm
        .complete(&messages, model, 0.3)
        .await?
        .trim()
        .to_string();

    let parsed = match parse_rule_extraction_response(&content) {
        Ok(rules) => rules,
        Err(e) => {
            let detail = format!("{} — raw: {:.200}", e, content);
            tracing::warn!("Failed to parse LLM rule extraction output: {}", detail);
            let _ = block_in_place(|| {
                let conn = crate::feedback::open_evolution_db(chat_root)?;
                let _ = crate::log_evolution_event(
                    &conn,
                    chat_root,
                    "rule_extraction_parse_failed",
                    "",
                    &detail,
                    "",
                );
                Ok::<_, anyhow::Error>(())
            });
            return Ok(Vec::new());
        }
    };
    if parsed.is_empty() {
        return Ok(Vec::new());
    }

    let mut valid_rules = Vec::new();
    for rule in parsed {
        if let Err(e) = gatekeeper_l3_content(&rule.instruction) {
            tracing::warn!("L3 rejected rule {}: {}", rule.id, e);
            continue;
        }
        if rule.priority < 50 || rule.priority > 79 {
            tracing::warn!(
                "Rule {} has invalid priority {} (must be 50-79), adjusting",
                rule.id,
                rule.priority
            );
            let mut r = rule;
            r.priority = r.priority.clamp(50, 79);
            valid_rules.push(r);
        } else {
            valid_rules.push(rule);
        }
    }

    if valid_rules.is_empty() {
        return Ok(Vec::new());
    }

    let mut all_rules = existing_rules;
    let mut changes = Vec::new();

    let available_slots = 50_usize.saturating_sub(all_rules.len());
    let to_add = valid_rules.into_iter().take(available_slots);

    for new_rule in to_add {
        if all_rules.iter().any(|r| r.id == new_rule.id) {
            continue;
        }
        changes.push(("rule_added".to_string(), new_rule.id.clone()));
        all_rules.push(new_rule);
    }

    if !changes.is_empty() {
        let path = chat_root.join("prompts").join("rules.json");
        if !gatekeeper_l1_path(chat_root, &path, None) {
            bail!("Gatekeeper L1: rules.json path outside allowed directories");
        }
        let json = serde_json::to_string_pretty(&all_rules)?;
        atomic_write(&path, &json)?;
        tracing::info!("Added {} new rules via evolution", changes.len());
    }

    Ok(changes)
}

fn parse_rule_extraction_response(content: &str) -> Result<Vec<PlanningRule>> {
    let json_str = extract_json_block(content);

    let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        crate::Error::validation(format!("Failed to parse rule extraction JSON: {}", e))
    })?;

    let rules_array = parsed
        .get("rules")
        .and_then(|v| v.as_array())
        .ok_or_else(|| crate::Error::validation("No 'rules' array in response"))?;

    let mut rules = Vec::new();
    for rule_val in rules_array {
        let id = rule_val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let instruction = rule_val
            .get("instruction")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if instruction.is_empty() || instruction.len() > 200 {
            continue;
        }
        let priority = rule_val
            .get("priority")
            .and_then(|v| v.as_u64())
            .unwrap_or(65) as u32;
        let keywords: Vec<String> = rule_val
            .get("keywords")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let context_keywords: Vec<String> = rule_val
            .get("context_keywords")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let tool_hint = rule_val
            .get("tool_hint")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .map(String::from);

        rules.push(PlanningRule {
            id,
            priority,
            keywords,
            context_keywords,
            tool_hint,
            instruction,
            mutable: true,
            origin: "evolved".to_string(),
            reusable: false,
            effectiveness: None,
            trigger_count: None,
        });
    }

    Ok(rules)
}

async fn generate_examples_from_data<L: EvolutionLlm>(
    chat_root: &Path,
    example_data: Option<(Option<String>, Option<String>, i64)>,
    llm: &L,
    model: &str,
) -> Result<Vec<(String, String)>> {
    let (task_desc, tools_json, elapsed_ms) = match example_data {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let task_desc = task_desc.unwrap_or_default();
    if task_desc.is_empty() {
        return Ok(Vec::new());
    }

    let examples_path = chat_root.join("prompts").join("examples.json");
    let existing_examples: Vec<PlanningExample> = if examples_path.exists() {
        skilllite_fs::read_file(&examples_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if existing_examples.len() >= 25 {
        return Ok(Vec::new());
    }

    let existing_summary = existing_examples
        .iter()
        .map(|e| format!("- {}: {}", e.id, e.task_pattern))
        .collect::<Vec<_>>()
        .join("\n");

    let tool_sequence = tools_json.unwrap_or_else(|| "[]".to_string());
    let rules_used = "N/A".to_string();

    let prompt = EXAMPLE_GENERATION_PROMPT
        .replace("{{existing_examples_summary}}", &existing_summary)
        .replace("{{task_description}}", &task_desc)
        .replace("{{tool_sequence}}", &tool_sequence)
        .replace("{{rules_used}}", &rules_used)
        .replace("{{elapsed_ms}}", &elapsed_ms.to_string());

    let messages = vec![EvolutionMessage::user(&prompt)];
    let content = llm
        .complete(&messages, model, 0.3)
        .await?
        .trim()
        .to_string();

    let example = match parse_example_response(&content) {
        Ok(ex) => ex,
        Err(e) => {
            let detail = format!("{} — raw: {:.200}", e, content);
            tracing::warn!("Failed to parse LLM example output: {}", detail);
            let _ = block_in_place(|| {
                let conn = crate::feedback::open_evolution_db(chat_root)?;
                let _ = crate::log_evolution_event(
                    &conn,
                    chat_root,
                    "example_generation_parse_failed",
                    "",
                    &detail,
                    "",
                );
                Ok::<_, anyhow::Error>(())
            });
            return Ok(Vec::new());
        }
    };
    let example = match example {
        Some(e) => e,
        None => return Ok(Vec::new()),
    };

    let combined = format!(
        "{} {} {}",
        example.task_pattern, example.plan_template, example.key_insight
    );
    if let Err(e) = gatekeeper_l3_content(&combined) {
        tracing::warn!("L3 rejected example {}: {}", example.id, e);
        return Ok(Vec::new());
    }

    if !gatekeeper_l1_path(chat_root, &examples_path, None) {
        bail!("Gatekeeper L1: examples.json path outside allowed directories");
    }

    let mut all_examples = existing_examples;
    if all_examples.iter().any(|e| e.id == example.id) {
        return Ok(Vec::new());
    }

    let change_id = example.id.clone();
    all_examples.push(example);

    let json = serde_json::to_string_pretty(&all_examples)?;
    atomic_write(&examples_path, &json)?;
    tracing::info!("Added new example: {}", change_id);

    Ok(vec![("example_added".to_string(), change_id)])
}

fn parse_example_response(content: &str) -> Result<Option<PlanningExample>> {
    let json_str = extract_json_block(content);

    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| crate::Error::validation(format!("Failed to parse example JSON: {}", e)))?;

    if let Some(skip) = parsed.get("skip_reason").and_then(|v| v.as_str()) {
        if !skip.is_empty() && skip != "null" {
            return Ok(None);
        }
    }

    let example_val = parsed
        .get("example")
        .ok_or_else(|| crate::Error::validation("No 'example' field in response"))?;

    let id = example_val
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let task_pattern = example_val
        .get("task_pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let plan_template = example_val
        .get("plan_template")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let key_insight = example_val
        .get("key_insight")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if id.is_empty() || task_pattern.is_empty() || plan_template.is_empty() {
        return Ok(None);
    }

    Ok(Some(PlanningExample {
        id,
        task_pattern,
        plan_template,
        key_insight,
        origin: "evolved".to_string(),
    }))
}

/// Retire rules with effectiveness below threshold and sufficient trigger history.
/// Only mutable (evolved/external) rules are retired; seed rules are preserved.
/// Returns `(change_type, rule_id)` pairs for changelog.
fn retire_low_effectiveness_rules_with_conn(
    chat_root: &Path,
    txn_id: &str,
    conn: &Connection,
) -> Result<Vec<(String, String)>> {
    let rules_path = chat_root.join("prompts").join("rules.json");
    if !rules_path.exists() {
        return Ok(Vec::new());
    }
    if !gatekeeper_l1_path(chat_root, &rules_path, None) {
        bail!("Gatekeeper L1: rules.json path outside allowed directories");
    }
    let content = skilllite_fs::read_file(&rules_path)?;
    let rules: Vec<PlanningRule> = serde_json::from_str(&content)?;

    let mut to_retire: Vec<(String, String)> = Vec::new();
    let mut kept: Vec<PlanningRule> = Vec::new();

    for rule in rules {
        if !rule.mutable {
            kept.push(rule);
            continue;
        }
        let eff = compute_effectiveness(conn, &rule.id).unwrap_or(-1.0);
        if eff < 0.0 {
            kept.push(rule);
            continue;
        }
        let trigger_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM decision_rules WHERE rule_id = ?1",
                params![rule.id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if eff < RETIRE_EFFECTIVENESS_THRESHOLD && trigger_count >= RETIRE_MIN_TRIGGER_COUNT {
            let reason = format!(
                "effectiveness {:.0}% < {:.0}% threshold, trigger_count {}",
                eff * 100.0,
                RETIRE_EFFECTIVENESS_THRESHOLD * 100.0,
                trigger_count
            );
            let _ = crate::log_evolution_event(
                conn,
                chat_root,
                "rule_retired",
                &rule.id,
                &reason,
                txn_id,
            );
            tracing::info!("Retired rule '{}': {}", rule.id, reason);
            to_retire.push(("rule_retired".to_string(), rule.id));
        } else {
            kept.push(rule);
        }
    }

    if to_retire.is_empty() {
        return Ok(Vec::new());
    }

    let json = serde_json::to_string_pretty(&kept)?;
    atomic_write(&rules_path, &json)?;

    Ok(to_retire)
}

pub fn update_reusable_status(conn: &Connection, chat_root: &Path) -> Result<()> {
    let rules_path = chat_root.join("prompts").join("rules.json");
    if !rules_path.exists() {
        return Ok(());
    }

    let content = skilllite_fs::read_file(&rules_path)?;
    let mut rules: Vec<PlanningRule> = serde_json::from_str(&content)?;

    let mut changed = false;
    for rule in rules.iter_mut() {
        if !rule.mutable {
            continue;
        }

        let eff = compute_effectiveness(conn, &rule.id)?;
        if eff < 0.0 {
            continue;
        }

        let trigger_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM decision_rules WHERE rule_id = ?1",
                params![rule.id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        rule.effectiveness = Some(eff);
        rule.trigger_count = Some(trigger_count as u32);

        if !rule.reusable && eff >= 0.7 && trigger_count >= 5 {
            rule.reusable = true;
            changed = true;
        } else if rule.reusable && eff < 0.5 {
            rule.reusable = false;
            changed = true;
        }
    }

    if changed {
        let json = serde_json::to_string_pretty(&rules)?;
        atomic_write(&rules_path, &json)?;
    }

    Ok(())
}

/// Row ids whose text is loaded for rule extraction / example generation (subset of a proposal window).
pub(crate) fn decision_ids_read_for_prompt_evolution(conn: &Connection) -> Result<Vec<i64>> {
    use std::collections::HashSet;

    let mut seen: HashSet<i64> = HashSet::new();
    let mut out: Vec<i64> = Vec::new();
    let rule_limit = prompt_rule_summary_limit();
    let push_ids = |conn: &Connection,
                    cond: &str,
                    lim: i64,
                    seen: &mut HashSet<i64>,
                    out: &mut Vec<i64>|
     -> Result<()> {
        let sql = format!(
            "SELECT id FROM decisions WHERE {cond} AND task_description IS NOT NULL ORDER BY ts DESC LIMIT {lim}"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
        for r in rows.filter_map(|x| x.ok()) {
            if seen.insert(r) {
                out.push(r);
            }
        }
        Ok(())
    };

    push_ids(
        conn,
        "evolved = 0 AND task_completed = 1 AND replans = 0 AND failed_tools = 0",
        rule_limit,
        &mut seen,
        &mut out,
    )?;
    push_ids(
        conn,
        "evolved = 0 AND (replans > 0 OR failed_tools > 0)",
        rule_limit,
        &mut seen,
        &mut out,
    )?;

    let min_tools = prompt_example_min_tools();
    let example_sql = format!(
        "SELECT id FROM decisions WHERE evolved = 0 AND task_completed = 1 AND replans = 0
               AND failed_tools = 0 AND total_tools >= {min_tools} AND task_description IS NOT NULL
         ORDER BY total_tools DESC LIMIT 1"
    );
    if let Ok(id) = conn.query_row(&example_sql, [], |row| row.get::<_, i64>(0)) {
        if seen.insert(id) {
            out.push(id);
        }
    }

    Ok(out)
}

fn query_decisions_summary(conn: &Connection, successful: bool, limit: i64) -> Result<String> {
    let condition = if successful {
        "evolved = 0 AND task_completed = 1 AND replans = 0 AND failed_tools = 0"
    } else {
        "evolved = 0 AND (replans > 0 OR failed_tools > 0)"
    };

    let sql = format!(
        "SELECT task_description, total_tools, failed_tools, replans, elapsed_ms
         FROM decisions WHERE {} AND task_description IS NOT NULL
         ORDER BY ts DESC LIMIT {}",
        condition, limit
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let desc: String = row.get(0)?;
            let total: i64 = row.get(1)?;
            let failed: i64 = row.get(2)?;
            let replans: i64 = row.get(3)?;
            let elapsed: i64 = row.get(4)?;
            Ok(format!(
                "- 任务: {} | 工具调用: {} (失败: {}) | replan: {} | 耗时: {}ms",
                desc, total, failed, replans, elapsed
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

pub fn extract_json_block(content: &str) -> String {
    let content = crate::strip_think_blocks(content.trim());

    if let Some(start) = content.find("```json") {
        let json_start = start + 7;
        if let Some(end) = content[json_start..].find("```") {
            return content[json_start..json_start + end].trim().to_string();
        }
    }

    if let Some(start) = content.find("```") {
        let block_start = start + 3;
        let actual_start = content[block_start..]
            .find('\n')
            .map(|n| block_start + n + 1)
            .unwrap_or(block_start);
        if let Some(end) = content[actual_start..].find("```") {
            return content[actual_start..actual_start + end].trim().to_string();
        }
    }

    if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
        if start < end {
            return content[start..=end].to_string();
        }
    }

    content.to_string()
}

#[cfg(test)]
mod extract_json_tests {
    use super::extract_json_block;

    #[test]
    fn extract_json_block_fenced_json() {
        let s = "intro\n```json\n{\"a\":1}\n```\ntrailer";
        assert_eq!(extract_json_block(s), "{\"a\":1}");
    }

    #[test]
    fn extract_json_block_brace_span() {
        let s = "prefix {\"x\": true} suffix";
        assert_eq!(extract_json_block(s), "{\"x\": true}");
    }

    #[test]
    fn extract_json_block_plain_after_strip_think() {
        let s = "<think>\n</redacted_thinking>\n{\"k\":\"v\"}";
        assert_eq!(extract_json_block(s), "{\"k\":\"v\"}");
    }
}
