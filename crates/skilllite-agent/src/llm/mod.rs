//! LLM HTTP client for chat completions with tool calling.
//!
//! Supports two API formats:
//!   - **OpenAI-compatible**: `/chat/completions` (GPT-4, DeepSeek, Qwen, etc.)
//!   - **Claude Native**: `/v1/messages` (Anthropic Claude)
//!
//! Auto-detects which API to use based on model name or API base URL.
//!
//! Ported from Python `AgenticLoop._call_openai` / `_call_claude`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::{
    ChatMessage, EventSink, ToolCall, ToolDefinition, ToolFormat, safe_truncate,
};

mod openai;
mod claude;

#[cfg(test)]
mod tests;

/// Detect API format from model name or API base.
pub fn detect_tool_format(model: &str, api_base: &str) -> ToolFormat {
    let model_lower = model.to_lowercase();
    let base_lower = api_base.to_lowercase();

    if model_lower.starts_with("claude")
        || base_lower.contains("anthropic")
        || base_lower.contains("claude")
    {
        ToolFormat::Claude
    } else {
        ToolFormat::OpenAI
    }
}

/// LLM client supporting both OpenAI and Claude API formats.
pub struct LlmClient {
    http: reqwest::Client,
    api_base: String,
    api_key: String,
}

impl LlmClient {
    pub fn new(api_base: &str, api_key: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            api_base: api_base.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    /// Non-streaming chat completion call (auto-routes based on model/api_base).
    pub async fn chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
    ) -> Result<ChatCompletionResponse> {
        let format = detect_tool_format(model, &self.api_base);
        match format {
            ToolFormat::Claude => self.claude_chat_completion(model, messages, tools, temperature).await,
            ToolFormat::OpenAI => self.openai_chat_completion(model, messages, tools, temperature).await,
        }
    }

    /// Streaming chat completion call (auto-routes based on model/api_base).
    pub async fn chat_completion_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
        event_sink: &mut dyn EventSink,
    ) -> Result<ChatCompletionResponse> {
        let format = detect_tool_format(model, &self.api_base);
        match format {
            ToolFormat::Claude => {
                self.claude_chat_completion_stream(model, messages, tools, temperature, event_sink)
                    .await
            }
            ToolFormat::OpenAI => {
                self.openai_chat_completion_stream(model, messages, tools, temperature, event_sink)
                    .await
            }
        }
    }

    /// Embed text(s) using OpenAI-compatible /embeddings API.
    /// Returns one embedding vector per input string. Used when memory_vector feature is enabled.
    #[allow(dead_code)]
    pub async fn embed(&self, model: &str, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/embeddings", self.api_base);
        let input: Value = if texts.len() == 1 {
            json!(texts[0])
        } else {
            json!(texts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        let body = json!({ "model": model, "input": input });
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Embedding API request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Embedding API error ({}): {}", status, body_text);
        }
        let json: Value = resp.json().await.context("Failed to parse embedding response")?;
        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .context("Missing 'data' in embedding response")?;
        let mut embeddings = Vec::with_capacity(data.len());
        for item in data {
            let emb = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .context("Missing 'embedding' in embedding item")?;
            let vec: Vec<f32> = emb
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            embeddings.push(vec);
        }
        Ok(embeddings)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // OpenAI-compatible API
    // ═══════════════════════════════════════════════════════════════════════════
}

// ─── Response types ─────────────────────────────────────────────────────────
// Fields id/model/usage/index/finish_reason/role are required for API deserialization
// but not read by our code.

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Choice {
    pub index: u32,
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChoiceMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}


// ─── Helpers ────────────────────────────────────────────────────────────────

/// Check if an error is a context overflow (token limit exceeded).
/// Ported from Python `_is_context_overflow_error`.
pub fn is_context_overflow_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("context_length_exceeded")
        || lower.contains("maximum context length")
        || lower.contains("token limit")
        || lower.contains("too many tokens")
        || lower.contains("context window")
        || lower.contains("max_tokens")
}

/// Truncate all tool result messages in place to reduce context size.
/// Ported from Python `_truncate_tool_messages_in_place`.
pub fn truncate_tool_messages(messages: &mut [ChatMessage], max_chars: usize) {
    for msg in messages.iter_mut() {
        if msg.role == "tool" {
            if let Some(ref mut content) = msg.content {
                if content.len() > max_chars {
                    let truncated = format!(
                        "{}...\n[truncated: {} chars → {}]",
                        safe_truncate(content, max_chars),
                        content.len(),
                        max_chars
                    );
                    *content = truncated;
                }
            }
        }
    }
}
