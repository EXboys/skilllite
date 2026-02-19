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
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::{
    ChatMessage, EventSink, FunctionCall, ToolCall, ToolDefinition, ToolFormat, safe_truncate,
    parse_claude_tool_calls, get_max_tokens,
};

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

    // ═══════════════════════════════════════════════════════════════════════════
    // OpenAI-compatible API
    // ═══════════════════════════════════════════════════════════════════════════

    async fn openai_chat_completion(
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

    async fn openai_chat_completion_stream(
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
    ///   - No role="tool" messages
    fn convert_messages_for_claude(
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

    async fn claude_chat_completion(
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

    async fn claude_chat_completion_stream(
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
    fn convert_claude_response(response: Value, model: &str) -> Result<ChatCompletionResponse> {
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
    async fn accumulate_claude_stream(
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

    /// Parse OpenAI SSE stream and accumulate into a complete response.
    /// Text chunks are streamed to event_sink immediately.
    async fn accumulate_openai_stream(
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_tool_format_openai() {
        assert_eq!(
            detect_tool_format("gpt-4o", "https://api.openai.com/v1"),
            ToolFormat::OpenAI
        );
        assert_eq!(
            detect_tool_format("deepseek-chat", "https://api.deepseek.com/v1"),
            ToolFormat::OpenAI
        );
        assert_eq!(
            detect_tool_format("qwen-turbo", "https://dashscope.aliyuncs.com/v1"),
            ToolFormat::OpenAI
        );
    }

    #[test]
    fn test_detect_tool_format_claude() {
        assert_eq!(
            detect_tool_format("claude-3-5-sonnet-20241022", "https://api.anthropic.com"),
            ToolFormat::Claude
        );
        assert_eq!(
            detect_tool_format("claude-3-opus", "https://custom.proxy.com"),
            ToolFormat::Claude
        );
        assert_eq!(
            detect_tool_format("gpt-4o", "https://anthropic-proxy.example.com/v1"),
            ToolFormat::Claude
        );
    }

    #[test]
    fn test_convert_messages_for_claude_basic() {
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
        ];

        let (system, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

        assert_eq!(system, Some("You are helpful.".to_string()));
        assert_eq!(claude_msgs.len(), 2); // user + assistant (system extracted)

        assert_eq!(claude_msgs[0]["role"], "user");
        assert_eq!(claude_msgs[0]["content"], "Hello");

        assert_eq!(claude_msgs[1]["role"], "assistant");
        let content = claude_msgs[1]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hi there!");
    }

    #[test]
    fn test_convert_messages_for_claude_tool_calls() {
        let tool_call = ToolCall {
            id: "tc_123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "read_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        };

        let messages = vec![
            ChatMessage::system("System prompt"),
            ChatMessage::user("Read the file"),
            ChatMessage::assistant_with_tool_calls(Some("Let me read that."), vec![tool_call]),
            ChatMessage::tool_result("tc_123", "File contents here"),
        ];

        let (system, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

        assert!(system.is_some());
        assert_eq!(claude_msgs.len(), 3); // user, assistant (with tool_use), user (with tool_result)

        // Check assistant has both text and tool_use blocks
        let assistant_content = claude_msgs[1]["content"].as_array().unwrap();
        assert_eq!(assistant_content.len(), 2);
        assert_eq!(assistant_content[0]["type"], "text");
        assert_eq!(assistant_content[1]["type"], "tool_use");
        assert_eq!(assistant_content[1]["id"], "tc_123");
        assert_eq!(assistant_content[1]["name"], "read_file");

        // Check tool result is wrapped as user message
        let tool_result_msg = &claude_msgs[2];
        assert_eq!(tool_result_msg["role"], "user");
        let result_content = tool_result_msg["content"].as_array().unwrap();
        assert_eq!(result_content[0]["type"], "tool_result");
        assert_eq!(result_content[0]["tool_use_id"], "tc_123");
        assert_eq!(result_content[0]["content"], "File contents here");
    }

    #[test]
    fn test_convert_messages_for_claude_multiple_tool_results() {
        let tc1 = ToolCall {
            id: "tc_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "tool_a".to_string(),
                arguments: "{}".to_string(),
            },
        };
        let tc2 = ToolCall {
            id: "tc_2".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "tool_b".to_string(),
                arguments: "{}".to_string(),
            },
        };

        let messages = vec![
            ChatMessage::system("sys"),
            ChatMessage::user("do both"),
            ChatMessage::assistant_with_tool_calls(None, vec![tc1, tc2]),
            ChatMessage::tool_result("tc_1", "result a"),
            ChatMessage::tool_result("tc_2", "result b"),
        ];

        let (_, claude_msgs) = LlmClient::convert_messages_for_claude(&messages);

        // Multiple tool results should be batched into one user message
        assert_eq!(claude_msgs.len(), 3); // user, assistant, user(tool_results)

        let result_content = claude_msgs[2]["content"].as_array().unwrap();
        assert_eq!(result_content.len(), 2);
        assert_eq!(result_content[0]["tool_use_id"], "tc_1");
        assert_eq!(result_content[1]["tool_use_id"], "tc_2");
    }

    #[test]
    fn test_convert_claude_response() {
        let response = json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Here is the result."}
            ],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let result = LlmClient::convert_claude_response(response, "claude-3-5-sonnet").unwrap();

        assert_eq!(result.choices.len(), 1);
        assert_eq!(
            result.choices[0].message.content,
            Some("Here is the result.".to_string())
        );
        assert!(result.choices[0].message.tool_calls.is_none());
        assert_eq!(result.choices[0].finish_reason, Some("stop".to_string()));
        assert!(result.usage.is_some());
        assert_eq!(result.usage.unwrap().total_tokens, 150);
    }

    #[test]
    fn test_convert_claude_response_with_tool_use() {
        let response = json!({
            "id": "msg_456",
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "text", "text": "Let me read that file."},
                {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "read_file",
                    "input": {"path": "hello.txt"}
                }
            ],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 50, "output_tokens": 30}
        });

        let result = LlmClient::convert_claude_response(response, "claude-3-5-sonnet").unwrap();

        assert_eq!(
            result.choices[0].message.content,
            Some("Let me read that file.".to_string())
        );
        let tool_calls = result.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "toolu_01");
        assert_eq!(tool_calls[0].function.name, "read_file");
        assert!(tool_calls[0].function.arguments.contains("hello.txt"));
        assert_eq!(
            result.choices[0].finish_reason,
            Some("tool_calls".to_string())
        );
    }

    #[test]
    fn test_is_context_overflow_error() {
        assert!(is_context_overflow_error("context_length_exceeded"));
        assert!(is_context_overflow_error("Maximum context length exceeded"));
        assert!(is_context_overflow_error("too many tokens in request"));
        assert!(!is_context_overflow_error("rate limit exceeded"));
        assert!(!is_context_overflow_error("invalid api key"));
    }
}
