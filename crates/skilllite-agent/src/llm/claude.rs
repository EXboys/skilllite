//! Anthropic Claude API implementation.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::types::{
    ChatMessage, EventSink, FunctionCall, ToolCall, ToolDefinition,
    parse_claude_tool_calls, get_max_tokens,
};

use super::{ChatCompletionResponse, Choice, ChoiceMessage, Usage, LlmClient};

impl LlmClient {
    pub(super) fn convert_messages_for_claude(
        messages: &[ChatMessage],
    ) -> (Option<String>, Vec<Value>) {
        let mut system_prompt = None;
        let mut claude_messages: Vec<Value> = Vec::new();

        // Collect pending tool results to batch into a single user message
        let mut pending_tool_results: Vec<Value> = Vec::new();

        for msg in messages {
            // Flush pending tool results before any non-tool message
            if msg.role != "tool" && !pending_tool_results.is_empty() {
                claude_messages.push(json!({
                    "role": "user",
                    "content": pending_tool_results.clone()
                }));
                pending_tool_results.clear();
            }

            match msg.role.as_str() {
                "system" => {
                    // Merge system messages into one
                    if let Some(ref content) = msg.content {
                        system_prompt = Some(match system_prompt {
                            Some(existing) => format!("{}\n\n{}", existing, content),
                            None => content.clone(),
                        });
                    }
                }
                "user" => {
                    claude_messages.push(json!({
                        "role": "user",
                        "content": msg.content.as_deref().unwrap_or("")
                    }));
                }
                "assistant" => {
                    let mut content_blocks: Vec<Value> = Vec::new();

                    // Text content
                    if let Some(ref text) = msg.content {
                        if !text.is_empty() {
                            content_blocks.push(json!({
                                "type": "text",
                                "text": text
                            }));
                        }
                    }

                    // Tool use blocks
                    if let Some(ref tool_calls) = msg.tool_calls {
                        for tc in tool_calls {
                            let input: Value = serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(json!({}));
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.function.name,
                                "input": input
                            }));
                        }
                    }

                    if !content_blocks.is_empty() {
                        claude_messages.push(json!({
                            "role": "assistant",
                            "content": content_blocks
                        }));
                    }
                }
                "tool" => {
                    // Accumulate tool results to batch into one user message
                    let tool_call_id = msg.tool_call_id.as_deref().unwrap_or("");
                    pending_tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": msg.content.as_deref().unwrap_or("")
                    }));
                }
                _ => {}
            }
        }

        // Flush any remaining tool results
        if !pending_tool_results.is_empty() {
            claude_messages.push(json!({
                "role": "user",
                "content": pending_tool_results
            }));
        }

        (system_prompt, claude_messages)
    }

    pub(super) async fn claude_chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches("/v1"));

        let (system_prompt, claude_messages) = Self::convert_messages_for_claude(messages);

        let mut body = json!({
            "model": model,
            "max_tokens": get_max_tokens(),
            "messages": claude_messages,
        });

        if let Some(system) = &system_prompt {
            body["system"] = json!(system);
        }
        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let claude_tools: Vec<Value> = tools.iter().map(|t| t.to_claude_format()).collect();
                body["tools"] = json!(claude_tools);
            }
        }

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Claude API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error ({}): {}", status, body_text);
        }

        let response: Value = resp.json().await.context("Failed to parse Claude response")?;
        Self::convert_claude_response(response, model)
    }

    pub(super) async fn claude_chat_completion_stream(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
        event_sink: &mut dyn EventSink,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/v1/messages", self.api_base.trim_end_matches("/v1"));

        let (system_prompt, claude_messages) = Self::convert_messages_for_claude(messages);

        let mut body = json!({
            "model": model,
            "max_tokens": get_max_tokens(),
            "messages": claude_messages,
            "stream": true,
        });

        if let Some(system) = &system_prompt {
            body["system"] = json!(system);
        }
        if let Some(temp) = temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let claude_tools: Vec<Value> = tools.iter().map(|t| t.to_claude_format()).collect();
                body["tools"] = json!(claude_tools);
            }
        }

        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Claude API request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Claude API error ({}): {}", status, body_text);
        }

        self.accumulate_claude_stream(resp, model, event_sink).await
    }

    /// Convert a non-streaming Claude response into our unified format.
    pub(super) fn convert_claude_response(response: Value, model: &str) -> Result<ChatCompletionResponse> {
        let content_blocks = response
            .get("content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for block in &content_blocks {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_content.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let tc = parse_claude_tool_calls(&[block.clone()]);
                    tool_calls.extend(tc);
                }
                _ => {}
            }
        }

        let stop_reason = response
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .map(|s| match s {
                "end_turn" => "stop",
                "tool_use" => "tool_calls",
                other => other,
            })
            .map(String::from);

        let usage = response.get("usage").and_then(|u| {
            Some(Usage {
                prompt_tokens: u.get("input_tokens")?.as_u64()?,
                completion_tokens: u.get("output_tokens")?.as_u64()?,
                total_tokens: u.get("input_tokens")?.as_u64()? + u.get("output_tokens")?.as_u64()?,
            })
        });

        Ok(ChatCompletionResponse {
            id: response
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string(),
            model: model.to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChoiceMessage {
                    role: "assistant".to_string(),
                    content: if text_content.is_empty() {
                        None
                    } else {
                        Some(text_content)
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                },
                finish_reason: stop_reason,
            }],
            usage,
        })
    }

    /// Parse Claude SSE stream and accumulate into a unified response.
    ///
    /// Claude SSE events:
    ///   - `message_start` → message metadata
    ///   - `content_block_start` → new text or tool_use block
    ///   - `content_block_delta` → incremental text or tool input
    ///   - `content_block_stop` → block complete
    ///   - `message_delta` → stop_reason, usage
    ///   - `message_stop` → stream complete
    pub(super) async fn accumulate_claude_stream(
        &self,
        resp: reqwest::Response,
        model: &str,
        event_sink: &mut dyn EventSink,
    ) -> Result<ChatCompletionResponse> {
        let mut text_content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_input = String::new();
        let mut in_tool_use = false;
        let mut stop_reason = None;
        let mut usage = None;

        let mut buffer = String::new();
        let mut stream = resp.bytes_stream();
        let mut current_event_type = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.context("Claude stream chunk error")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // Claude uses "event: <type>" followed by "data: <json>"
                if let Some(event_type) = line.strip_prefix("event: ") {
                    current_event_type = event_type.trim().to_string();
                    continue;
                }

                if !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                let chunk: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match current_event_type.as_str() {
                    "content_block_start" => {
                        if let Some(block) = chunk.get("content_block") {
                            match block.get("type").and_then(|t| t.as_str()) {
                                Some("tool_use") => {
                                    in_tool_use = true;
                                    current_tool_id = block
                                        .get("id")
                                        .and_then(|i| i.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_name = block
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    current_tool_input.clear();
                                }
                                _ => {
                                    in_tool_use = false;
                                }
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = chunk.get("delta") {
                            match delta.get("type").and_then(|t| t.as_str()) {
                                Some("text_delta") => {
                                    if let Some(text) = delta.get("text").and_then(|t| t.as_str())
                                    {
                                        text_content.push_str(text);
                                        event_sink.on_text_chunk(text);
                                    }
                                }
                                Some("input_json_delta") => {
                                    if let Some(json_part) =
                                        delta.get("partial_json").and_then(|j| j.as_str())
                                    {
                                        current_tool_input.push_str(json_part);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_stop" => {
                        if in_tool_use {
                            tool_calls.push(ToolCall {
                                id: current_tool_id.clone(),
                                call_type: "function".to_string(),
                                function: FunctionCall {
                                    name: current_tool_name.clone(),
                                    arguments: if current_tool_input.is_empty() {
                                        "{}".to_string()
                                    } else {
                                        current_tool_input.clone()
                                    },
                                },
                            });
                            in_tool_use = false;
                        }
                    }
                    "message_delta" => {
                        if let Some(delta) = chunk.get("delta") {
                            if let Some(sr) = delta.get("stop_reason").and_then(|s| s.as_str()) {
                                stop_reason = Some(match sr {
                                    "end_turn" => "stop".to_string(),
                                    "tool_use" => "tool_calls".to_string(),
                                    other => other.to_string(),
                                });
                            }
                        }
                        if let Some(u) = chunk.get("usage") {
                            usage = Some(Usage {
                                prompt_tokens: 0, // only available in message_start
                                completion_tokens: u
                                    .get("output_tokens")
                                    .and_then(|o| o.as_u64())
                                    .unwrap_or(0),
                                total_tokens: u
                                    .get("output_tokens")
                                    .and_then(|o| o.as_u64())
                                    .unwrap_or(0),
                            });
                        }
                    }
                    "message_stop" => {
                        // Stream complete
                    }
                    _ => {}
                }
            }
        }

        // Trailing newline after streamed text
        if !text_content.is_empty() {
            event_sink.on_text_chunk("\n");
        }

        Ok(ChatCompletionResponse {
            id: String::new(),
            model: model.to_string(),
            choices: vec![Choice {
                index: 0,
                message: ChoiceMessage {
                    role: "assistant".to_string(),
                    content: if text_content.is_empty() {
                        None
                    } else {
                        Some(text_content)
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                },
                finish_reason: stop_reason,
            }],
            usage,
        })
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // OpenAI SSE stream accumulator
    // ═══════════════════════════════════════════════════════════════════════════

}
