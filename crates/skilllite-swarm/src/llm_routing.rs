//! LLM-based capability inference for routing.
//!
//! When required_capabilities is empty, calls LLM to infer which capability tags
//! the task needs, then routes accordingly. Default: enabled when OPENAI_API_KEY
//! is set. Set SKILLLITE_SWARM_LLM_ROUTING=0 to disable (e.g. to save cost).

use serde::Deserialize;
use std::time::Duration;

const ENV_LLM_ROUTING: &str = "SKILLLITE_SWARM_LLM_ROUTING";
const TIMEOUT_SECS: u64 = 15;

/// Infer required capability tags from task description via LLM.
/// Returns empty vec on failure or when no specific capabilities needed.
/// Default: enabled. Set SKILLLITE_SWARM_LLM_ROUTING=0 to disable.
pub async fn infer_required_capabilities(
    task_description: &str,
    available_tags: &[String],
) -> Vec<String> {
    if available_tags.is_empty() {
        return vec![];
    }

    if std::env::var(ENV_LLM_ROUTING).as_deref() == Ok("0") {
        tracing::debug!("LLM routing disabled (SKILLLITE_SWARM_LLM_ROUTING=0)");
        return vec![];
    }

    let cfg = match skilllite_core::config::LlmConfig::try_from_env() {
        Some(c) => c,
        None => {
            tracing::info!(
                "LLM routing skipped: OPENAI_API_KEY not set. Tip: run from project dir (with .env) or use --skills-dir path/to/project/skills"
            );
            return vec![];
        }
    };

    let tags_str = available_tags.join(", ");
    let prompt = format!(
        r#"Task: {}
Available capability tags: {}

Which tag(s) from the list does this task need? Return a JSON array. Prefer routing to specialized nodes:
- Arithmetic/calculation (1+1, 计算, 加减乘除) → ["calc"] if available
- HTTP/API/网页 → ["web"]
- Browser automation → ["browser"]
- Data analysis → ["data"]
- Return [] ONLY for generic chat (greetings, general Q&A with no tool need).
JSON array:"#,
        task_description.trim(),
        tags_str
    );

    let url = format!("{}/chat/completions", cfg.api_base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": cfg.model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 64,
        "temperature": 0
    });

    let client = reqwest::Client::new();
    let resp = match client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&body)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                err = ?e,
                url = %url,
                "LLM routing request failed"
            );
            return vec![];
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::warn!("LLM routing API error {}: {}", status, text);
        return vec![];
    }

    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Option<Vec<Choice>>,
    }
    #[derive(Deserialize)]
    struct Choice {
        message: Option<Message>,
    }
    #[derive(Deserialize)]
    struct Message {
        content: Option<String>,
    }

    let chat: ChatResponse = match resp.json().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("LLM routing parse failed: {}", e);
            return vec![];
        }
    };

    let content = chat
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .unwrap_or_default();

    // Extract JSON array from response (may be wrapped in markdown)
    let content = content.trim();
    let json_start = content.find('[').unwrap_or(0);
    let json_end = content.rfind(']').map(|i| i + 1).unwrap_or(content.len());
    let json_str = content.get(json_start..json_end).unwrap_or("[]");

    let tags: Vec<String> = match serde_json::from_str(json_str) {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!("LLM routing JSON parse failed: {} (raw: {})", e, json_str);
            return vec![];
        }
    };

    // Filter to only tags that exist in available
    let avail: std::collections::HashSet<_> = available_tags.iter().map(|s| s.as_str()).collect();
    let filtered: Vec<String> = tags
        .iter()
        .filter(|t| avail.contains(t.as_str()))
        .cloned()
        .collect();

    if !filtered.is_empty() {
        tracing::info!(
            inferred = ?filtered,
            "LLM routing inferred required capabilities"
        );
    } else if !tags.is_empty() {
        tracing::debug!(
            raw = ?tags,
            "LLM returned tags not in available list, ignoring"
        );
    }

    filtered
}
