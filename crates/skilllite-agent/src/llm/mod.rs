//! LLM HTTP client for chat completions with tool calling.
//!
//! Supports two API formats:
//!   - **OpenAI-compatible**: `/chat/completions` (GPT-4, DeepSeek, Qwen, etc.)
//!   - **Claude Native**: `/v1/messages` (Anthropic Claude)
//!
//! Auto-detects which API to use based on model name or API base URL.
//!
//! Ported from Python `AgenticLoop._call_openai` / `_call_claude`.

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::{safe_truncate, ChatMessage, EventSink, ToolCall, ToolDefinition, ToolFormat};

mod claude;
mod openai;

/// Normalize image MIME for OpenAI / Claude vision payloads.
pub(crate) fn normalize_vision_media_type(mt: &str) -> Result<&'static str> {
    let t = mt.trim().to_ascii_lowercase();
    match t.as_str() {
        "image/png" | "png" => Ok("image/png"),
        "image/jpeg" | "image/jpg" | "jpeg" | "jpg" => Ok("image/jpeg"),
        "image/webp" | "webp" => Ok("image/webp"),
        "image/gif" | "gif" => Ok("image/gif"),
        _ => bail!("Unsupported image media_type: {}", mt),
    }
}

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

/// TCP 连接阶段超时。`api.minimax.io` 等域名常解析出多个 A 记录，其中个别 IP
/// 可能长期不可达；过短的超时便于在第一次握手卡住后尽快尝试下一个地址（与 curl 行为接近）。
const HTTP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl LlmClient {
    pub fn new(api_base: &str, api_key: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("build HTTP client for LLM")?;
        Ok(Self {
            http,
            api_base: api_base.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        })
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
            ToolFormat::Claude => {
                self.claude_chat_completion(model, messages, tools, temperature)
                    .await
            }
            ToolFormat::OpenAI => {
                self.openai_chat_completion(model, messages, tools, temperature)
                    .await
            }
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
    /// If custom_url and custom_key are provided, use them instead of self.api_base/self.api_key.
    ///
    /// Handles both OpenAI-standard format (`{"data": [{"embedding": [...]}]}`)
    /// and Dashscope native format (`{"output": {"embeddings": [{"embedding": [...]}]}}`).
    #[allow(dead_code)]
    pub async fn embed(
        &self,
        model: &str,
        texts: &[&str],
        custom_url: Option<&str>,
        custom_key: Option<&str>,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let api_base = custom_url.unwrap_or(&self.api_base);
        let api_key = custom_key.unwrap_or(&self.api_key);
        let base = api_base.trim_end_matches('/');
        let url = if api_base.to_lowercase().contains("minimax") {
            format!("{}/text/embeddings", base)
        } else {
            format!("{}/embeddings", base)
        };
        let input: Value = if texts.len() == 1 {
            json!(texts[0])
        } else {
            json!(texts.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        let body = json!({ "model": model, "input": input });
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Embedding API request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            bail!("{}", format_api_error(status, &body_text, "Embedding"));
        }
        let json: Value = resp
            .json()
            .await
            .context("Failed to parse embedding response")?;

        // Try OpenAI-standard format: {"data": [{"embedding": [...]}]}
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            return Self::extract_embeddings_from_items(data);
        }

        // Fallback: Dashscope native format: {"output": {"embeddings": [{"embedding": [...]}]}}
        if let Some(items) = json
            .get("output")
            .and_then(|o| o.get("embeddings"))
            .and_then(|e| e.as_array())
        {
            tracing::debug!("Embedding response uses Dashscope native format (output.embeddings)");
            return Self::extract_embeddings_from_items(items);
        }

        // Log the unexpected response shape for debugging
        let preview = serde_json::to_string(&json).unwrap_or_default();
        let preview = &preview[..preview.len().min(500)];
        bail!(
            "Unexpected embedding response format (no 'data' or 'output.embeddings'): {}",
            preview
        )
    }

    /// Extract embedding vectors from a JSON array of items, each containing an "embedding" field.
    fn extract_embeddings_from_items(items: &[Value]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(items.len());
        for item in items {
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
    /// Reasoning/thinking content returned separately by reasoning models
    /// (e.g. DeepSeek R1 via official API or vLLM with --reasoning-parser).
    /// When present, `content` already excludes the thinking — no tag stripping needed.
    #[serde(default)]
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Format a user-friendly error from an LLM API HTTP failure.
///
/// Extracts a concise error detail from the response body (tries JSON `error.message`
/// or `message` fields first, falls back to truncated raw text), then prepends a
/// human-readable hint based on the HTTP status code.
pub(crate) fn format_api_error(status: reqwest::StatusCode, body: &str, provider: &str) -> String {
    let detail = extract_error_detail(body);

    let hint = match status.as_u16() {
        401 => "API Key 无效或已过期，请在设置中检查 Key 是否正确",
        402 => "账户余额不足或付费套餐已过期",
        403 => "API Key 权限不足，请确认 Key 有权访问所选模型",
        404 => "API 端点不存在，请检查 API Base URL 和模型名称是否正确",
        429 => "请求频率超限 (Rate Limit)，请稍后重试",
        500..=599 => "模型服务端错误，请稍后重试",
        _ => "",
    };

    if hint.is_empty() {
        format!("{} API 错误 ({}): {}", provider, status, detail)
    } else {
        format!("{} API 错误 ({}): {} — {}", provider, status, hint, detail)
    }
}

/// Try to extract a concise error message from a JSON response body.
fn extract_error_detail(body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
        {
            return msg.to_string();
        }
        if let Some(msg) = json.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
    }
    if body.len() > 200 {
        format!("{}…", &body[..200])
    } else {
        body.to_string()
    }
}

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

/// Rough total payload size for context budgeting (chars, not tokens).
///
/// Sums message text, tool-call arguments, and a small per-image placeholder.
/// Does not include system prompt or tool definitions added inside the agent loop.
pub fn estimate_messages_chars(messages: &[ChatMessage]) -> usize {
    messages.iter().map(message_estimated_chars).sum()
}

fn message_estimated_chars(m: &ChatMessage) -> usize {
    let mut n = m.content.as_deref().map(str::len).unwrap_or(0);
    if let Some(calls) = &m.tool_calls {
        for c in calls {
            n = n.saturating_add(c.id.len());
            n = n.saturating_add(c.call_type.len());
            n = n.saturating_add(c.function.name.len());
            n = n.saturating_add(c.function.arguments.len());
        }
    }
    if let Some(imgs) = &m.images {
        // Vision payloads dominate token usage; base64 length is a usable proxy.
        for img in imgs {
            n = n.saturating_add(img.data_base64.len());
        }
    }
    n
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
