//! OpenAI-compatible HTTP client for chat completions with tool calling.
//!
//! Ported from Python `AgenticLoop._call_openai` / `_accumulate_openai_stream`.
//! Uses reqwest + SSE parsing. Supports any OpenAI-compatible API endpoint.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::{ChatMessage, EventSink, FunctionCall, ToolCall, ToolDefinition, safe_truncate};

/// LLM client wrapping an OpenAI-compatible chat/completions endpoint.
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

    /// Non-streaming chat completion call.
    pub async fn chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", self.api_base);

        let mut body = json!({
            "model": model,
            "messages": messages,
        });

        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = serde_json::to_value(tools)?;
            }
        }

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("LLM API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status, body_text);
        }

        let response: ChatCompletionResponse = resp
            .json()
            .await
            .context("Failed to parse LLM API response")?;

        Ok(response)
    }

    /// Streaming chat completion call.
    /// Forwards text chunks via event_sink and accumulates tool calls.
    /// Returns the complete response once the stream ends.
    pub async fn chat_completion_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
        event_sink: &mut dyn EventSink,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", self.api_base);

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = serde_json::to_value(tools)?;
            }
        }

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("LLM API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error ({}): {}", status, body_text);
        }

        // Accumulate streaming response — ported from Python _accumulate_openai_stream
        self.accumulate_stream(resp, event_sink).await
    }

    /// Parse SSE stream and accumulate into a complete response.
    /// Text chunks are streamed to event_sink immediately (matching Python SDK
    /// `_accumulate_openai_stream`). Tool call deltas are accumulated silently.
    async fn accumulate_stream(
        &self,
        resp: reqwest::Response,
        event_sink: &mut dyn EventSink,
    ) -> Result<ChatCompletionResponse> {
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut model = String::new();
        let mut finish_reason = None;
        let mut usage = None;

        // Buffer for incomplete SSE lines
        let mut buffer = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Stream chunk error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }

                let chunk: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Extract model name
                if model.is_empty() {
                    if let Some(m) = chunk.get("model").and_then(|v| v.as_str()) {
                        model = m.to_string();
                    }
                }

                // Extract usage if present (some APIs send it on the last chunk)
                if let Some(u) = chunk.get("usage") {
                    if !u.is_null() {
                        usage = serde_json::from_value(u.clone()).ok();
                    }
                }

                // Process choices
                if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        // Check finish_reason
                        if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                            finish_reason = Some(fr.to_string());
                        }

                        let delta = match choice.get("delta") {
                            Some(d) => d,
                            None => continue,
                        };

                        // Stream text content to user immediately.
                        // Matches Python SDK: stream_callback(delta.content)
                        if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                            content.push_str(text);
                            event_sink.on_text_chunk(text);
                        }

                        // Accumulate tool calls silently (deltas arrive by index)
                        if let Some(tc_deltas) =
                            delta.get("tool_calls").and_then(|v| v.as_array())
                        {
                            for tc_delta in tc_deltas {
                                let idx =
                                    tc_delta.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as usize;

                                // Ensure we have enough slots
                                while tool_calls.len() <= idx {
                                    tool_calls.push(ToolCall {
                                        id: String::new(),
                                        call_type: "function".to_string(),
                                        function: FunctionCall {
                                            name: String::new(),
                                            arguments: String::new(),
                                        },
                                    });
                                }

                                // Merge delta into accumulated tool call
                                if let Some(id) = tc_delta.get("id").and_then(|v| v.as_str()) {
                                    tool_calls[idx].id = id.to_string();
                                }
                                if let Some(func) = tc_delta.get("function") {
                                    if let Some(name) = func.get("name").and_then(|v| v.as_str())
                                    {
                                        tool_calls[idx].function.name.push_str(name);
                                    }
                                    if let Some(args) =
                                        func.get("arguments").and_then(|v| v.as_str())
                                    {
                                        tool_calls[idx].function.arguments.push_str(args);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Ensure newline after streamed text so logs don't collide.
        // Matches Python SDK: `if stream_callback and message.content: print()`
        if !content.is_empty() {
            event_sink.on_text_chunk("\n");
        }

        let message_content = if content.is_empty() {
            None
        } else {
            Some(content)
        };
        let message_tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        Ok(ChatCompletionResponse {
            id: String::new(),
            model,
            choices: vec![Choice {
                index: 0,
                message: ChoiceMessage {
                    role: "assistant".to_string(),
                    content: message_content,
                    tool_calls: message_tool_calls,
                },
                finish_reason,
            }],
            usage,
        })
    }
}

// ─── Response types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
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
