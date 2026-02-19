//! Planning rules generation: use LLM to generate planning rules from skill docs.
//!
//! During `skilllite init --use-llm`, scans `.skills/*/SKILL.md`, collects name/description/body,
//! calls LLM to generate PlanningRule JSON, merges with builtin rules, writes to `.skilllite/planning_rules.json`.

#[cfg(feature = "agent")]
use anyhow::{Context, Result};
#[cfg(feature = "agent")]
use std::collections::HashMap;
#[cfg(feature = "agent")]
use std::fs;
#[cfg(feature = "agent")]
use std::path::{Path, PathBuf};

#[cfg(feature = "agent")]
use crate::agent::planning_rules;
#[cfg(feature = "agent")]
use crate::agent::types::{AgentConfig, ChatMessage, PlanningRule};
#[cfg(feature = "agent")]
use crate::agent::llm::LlmClient;
#[cfg(feature = "agent")]
use crate::skill::metadata;

/// Extract body content from SKILL.md (content after the closing `---` of front matter).
/// Returns up to `max_chars` for summary.
#[cfg(feature = "agent")]
fn extract_skill_body(content: &str, max_chars: usize) -> String {
    let re = regex::Regex::new(r"(?s)^---\s*\n.*?\n---\s*\n")
        .expect("SKILL.md regex");
    let body = re.replace(content, "").trim().to_string();
    let summary: String = body.chars().take(max_chars).collect();
    if body.chars().count() > max_chars {
        format!("{}...", summary)
    } else {
        summary
    }
}

/// Collect skill docs: name, description, body summary.
#[cfg(feature = "agent")]
fn collect_skill_docs(skills_path: &Path, skill_names: &[String]) -> Vec<String> {
    const BODY_MAX: usize = 800;

    let mut docs = Vec::new();
    for name in skill_names {
        let skill_path = skills_path.join(name);
        let skill_md = skill_path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let content = match fs::read_to_string(&skill_md) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta = match metadata::parse_skill_metadata(&skill_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let body = extract_skill_body(&content, BODY_MAX);
        let desc = meta.description.as_deref().unwrap_or("").trim();
        docs.push(format!(
            "## Skill: {}\n- description: {}\n- body:\n{}",
            name,
            if desc.is_empty() { "(none)" } else { desc },
            if body.is_empty() { "(none)" } else { body.as_str() }
        ));
    }
    docs
}

/// Generate planning rules via LLM and write to `.skilllite/planning_rules.json`.
/// Only runs when `use_llm` is true, API key is configured, and skills exist.
/// Returns the output path on success.
#[cfg(feature = "agent")]
pub fn generate_planning_rules(
    workspace: &Path,
    skills_path: &Path,
    skill_names: &[String],
    use_llm: bool,
) -> Result<std::path::PathBuf> {
    if !use_llm || skill_names.is_empty() {
        return Err(anyhow::anyhow!("skipped"));
    }

    let config = AgentConfig::from_env();
    if config.api_key.is_empty() {
        return Err(anyhow::anyhow!("No API key, set BASE_URL and API_KEY"));
    }

    let docs = collect_skill_docs(skills_path, skill_names);
    if docs.is_empty() {
        return Err(anyhow::anyhow!("No skill docs to process"));
    }

    let system_prompt = r#"You are a task planning rule generator. Given skill documentation (name, description, body), output a JSON array of planning rules for the task planner.

Each rule has:
- id: unique string (e.g. skill name with underscore, e.g. "csdn_article", "xiaohongshu_writer")
- priority: number 50–95 (higher = more important)
- keywords: array of strings that trigger this rule (user message contains any → use this rule). Include Chinese and English variants.
- context_keywords: optional array for additional context (e.g. ["文章","博客"] for csdn)
- tool_hint: skill name to suggest (e.g. "csdn-article", "xiaohongshu-writer"), or null if LLM outputs directly without calling skill
- instruction: clear instruction for the planner, in English. Format: "**Topic**: When user asks X, do Y. Return task with tool_hint: \"skill-name\" or return []."

Generate rules ONLY for the skills provided. Output pure JSON array, no markdown fences, no other text.
Example output: [{"id":"csdn_article","priority":88,"keywords":["csdn","CSDN","csdn文章"],"context_keywords":["文章","博客"],"tool_hint":null,"instruction":"**CSDN 文章**: When user asks to write CSDN article, output Markdown directly. Do NOT return empty list."}]"#;

    let user_content = format!(
        "Generate planning rules for these skills:\n\n{}",
        docs.join("\n\n")
    );

    let client = LlmClient::new(&config.api_base, &config.api_key);
    let messages = vec![
        ChatMessage::system(system_prompt),
        ChatMessage::user(&user_content),
    ];

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let resp = rt.block_on(client.chat_completion(
        &config.model,
        &messages,
        None,
        Some(0.2),
    ))?;

    let raw = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "[]".to_string());

    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let generated: Vec<PlanningRule> = serde_json::from_str(cleaned)
        .with_context(|| format!("Failed to parse LLM planning rules JSON: {}", &raw[..raw.len().min(200)]))?;

    // Merge: builtin rules never overwritten. LLM only adds rules for skills in .skills/
    // that don't already have a builtin rule (same id).
    let mut by_id: HashMap<String, PlanningRule> = HashMap::new();
    for r in planning_rules::builtin_rules() {
        by_id.insert(r.id.clone(), r);
    }
    for r in generated {
        by_id.entry(r.id.clone()).or_insert(r);
    }
    let mut merged: Vec<PlanningRule> = by_id.into_values().collect();
    merged.sort_by(|a, b| b.priority.cmp(&a.priority));

    let out_dir = workspace.join(".skilllite");
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create {}", out_dir.display()))?;

    let out_path: PathBuf = out_dir.join("planning_rules.json");
    let json = serde_json::to_string_pretty(&merged)
        .context("Failed to serialize planning rules")?;
    fs::write(&out_path, json)
        .with_context(|| format!("Failed to write {}", out_path.display()))?;

    Ok(out_path)
}
