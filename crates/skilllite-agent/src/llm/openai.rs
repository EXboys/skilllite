//! OpenAI-compatible API implementation.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::types::{
    ChatMessage, EventSink, FunctionCall, ToolCall, ToolDefinition,
    get_max_tokens,
};

use super::{ChatCompletionResponse, Choice, ChoiceMessage, LlmClient};

impl LlmClient {
    pub(super) async fn openai_chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", self.api_base);

        let mut body = json!({
            "model": model,
            "max_tokens": get_max_tokens(),
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

    pub(super) async fn openai_chat_completion_stream(
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
            "max_tokens": get_max_tokens(),
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

        self.accumulate_openai_stream(resp, event_sink).await
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Claude Native API (Anthropic Messages API)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Convert OpenAI-format messages to Claude Messages API format.
    /// Claude differences:
    ///   - System prompt is a separate `system` field (not a message)
    ///   - Tool results are user messages with `tool_result` content blocks

    pub(super) async fn accumulate_openai_stream(
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
