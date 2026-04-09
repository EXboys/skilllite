//! OpenAI-compatible API implementation.

use crate::error::bail;
use crate::Result;
use anyhow::{anyhow, Context};
use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::types::{
    get_max_tokens, ChatMessage, EventSink, FunctionCall, ToolCall, ToolDefinition,
};

use super::{
    normalize_vision_media_type, ChatCompletionResponse, Choice, ChoiceMessage, LlmClient,
};

fn messages_contain_user_images(messages: &[ChatMessage]) -> bool {
    messages
        .iter()
        .any(|m| m.role == "user" && m.images.as_ref().is_some_and(|imgs| !imgs.is_empty()))
}

fn openai_user_content_value(msg: &ChatMessage) -> crate::Result<Value> {
    let imgs = msg.images.as_deref().filter(|s| !s.is_empty());
    let text = msg.content.as_deref().unwrap_or("").trim();
    if imgs.is_none() {
        return Ok(json!(text));
    }
    let mut parts = Vec::new();
    if !text.is_empty() {
        parts.push(json!({ "type": "text", "text": text }));
    }
    if let Some(slice) = imgs {
        for img in slice {
            let mt = normalize_vision_media_type(&img.media_type)?;
            parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", mt, img.data_base64.trim())
                }
            }));
        }
    }
    if parts.is_empty() {
        return Err(crate::Error::validation(
            "User message has image metadata but no usable image parts",
        ));
    }
    Ok(Value::Array(parts))
}

fn openai_api_message(msg: &ChatMessage) -> crate::Result<Value> {
    if msg.role == "user" {
        let content = openai_user_content_value(msg)?;
        return Ok(json!({ "role": "user", "content": content }));
    }
    serde_json::to_value(msg).map_err(|e| e.into())
}

fn chat_messages_to_openai_json(messages: &[ChatMessage]) -> crate::Result<Vec<Value>> {
    messages.iter().map(openai_api_message).collect()
}

/// MiniMax Coding Plan 不支持 `system` role (error 2013)。
/// 将所有 system 消息提取合并，注入到第一条 user 消息开头，保留指令语义。
fn transform_messages_for_minimax(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut system_parts: Vec<&str> = Vec::new();
    let mut rest: Vec<ChatMessage> = Vec::new();

    for msg in messages {
        if msg.role == "system" {
            if let Some(ref content) = msg.content {
                if !content.is_empty() {
                    system_parts.push(content);
                }
            }
        } else {
            rest.push(msg.clone());
        }
    }

    if system_parts.is_empty() {
        return rest;
    }

    let system_block = format!(
        "<instructions>\n{}\n</instructions>",
        system_parts.join("\n\n")
    );

    if let Some(first_user) = rest.iter_mut().find(|m| m.role == "user") {
        let original = first_user.content.as_deref().unwrap_or("");
        first_user.content = Some(format!("{}\n\n{}", system_block, original));
    } else {
        rest.insert(0, ChatMessage::user(&system_block));
    }

    rest
}

fn is_minimax(api_base: &str) -> bool {
    api_base.to_lowercase().contains("minimax")
}

fn openai_send_err(url: &str, e: reqwest::Error) -> anyhow::Error {
    anyhow!("LLM API request failed (POST {}): {}", url, e)
}

impl LlmClient {
    pub(super) async fn openai_chat_completion(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        temperature: Option<f64>,
    ) -> Result<ChatCompletionResponse> {
        let url = format!("{}/chat/completions", self.api_base);

        if is_minimax(&self.api_base) && messages_contain_user_images(messages) {
            bail!(
                "Image attachments are not supported for MiniMax Coding Plan. Use a vision-capable OpenAI-compatible model (e.g. gpt-4o) or another provider."
            );
        }

        let effective_messages: Vec<ChatMessage>;
        let msgs: &[ChatMessage] = if is_minimax(&self.api_base) {
            effective_messages = transform_messages_for_minimax(messages);
            &effective_messages
        } else {
            messages
        };

        let messages_json = chat_messages_to_openai_json(msgs)?;

        let mut body = json!({
            "model": model,
            "max_tokens": get_max_tokens(),
            "messages": messages_json,
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
            .map_err(|e| openai_send_err(&url, e))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            bail!("{}", super::format_api_error(status, &body_text, "LLM"));
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

        if is_minimax(&self.api_base) && messages_contain_user_images(messages) {
            bail!(
                "Image attachments are not supported for MiniMax Coding Plan. Use a vision-capable OpenAI-compatible model (e.g. gpt-4o) or another provider."
            );
        }

        let effective_messages: Vec<ChatMessage>;
        let msgs: &[ChatMessage] = if is_minimax(&self.api_base) {
            effective_messages = transform_messages_for_minimax(messages);
            &effective_messages
        } else {
            messages
        };

        let messages_json = chat_messages_to_openai_json(msgs)?;

        let mut body = json!({
            "model": model,
            "max_tokens": get_max_tokens(),
            "messages": messages_json,
            "stream": true,
        });
        // OpenAI / many proxies omit usage unless this is set; without it, completion_tokens
        // on the final chunk can be wrong or missing. Some vendors reject unknown fields.
        if !is_minimax(&self.api_base) {
            body["stream_options"] = json!({ "include_usage": true });
        }

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
            .map_err(|e| openai_send_err(&url, e))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            bail!("{}", super::format_api_error(status, &body_text, "LLM"));
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
        let mut reasoning_content = String::new();
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

                        // Accumulate reasoning_content from reasoning models (e.g. DeepSeek R1)
                        if let Some(rc) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                            reasoning_content.push_str(rc);
                        }

                        // Stream text content to user immediately.
                        // Matches Python SDK: stream_callback(delta.content)
                        if let Some(text) = delta.get("content").and_then(|v| v.as_str()) {
                            content.push_str(text);
                            event_sink.on_text_chunk(text);
                        }

                        // Accumulate tool calls silently (deltas arrive by index)
                        if let Some(tc_deltas) = delta.get("tool_calls").and_then(|v| v.as_array())
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
                                    if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
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
                    reasoning_content: if reasoning_content.is_empty() {
                        None
                    } else {
                        Some(reasoning_content)
                    },
                    tool_calls: message_tool_calls,
                },
                finish_reason,
            }],
            usage,
        })
    }
}

#[cfg(test)]
mod openai_attachment_tests {
    use super::*;
    use crate::types::{ChatMessage, UserImageAttachment};

    #[test]
    fn openai_user_with_image_uses_content_array() {
        let m = ChatMessage::user_with_images(
            "describe",
            Some(vec![UserImageAttachment {
                media_type: "image/png".to_string(),
                data_base64: "QUJD".to_string(),
            }]),
        );
        let v = openai_api_message(&m).expect("openai_api_message");
        assert_eq!(v["role"], "user");
        let parts = v["content"].as_array().expect("content array");
        assert!(parts.iter().any(|p| p.get("type") == Some(&json!("text"))));
        assert!(parts
            .iter()
            .any(|p| p.get("type") == Some(&json!("image_url"))));
    }
}
