//! Agent Chat RPC: JSON-Lines event stream protocol over stdio.
//!
//! **Entry**: `skilllite agent-rpc`
//!
//! **Scope**: Agent chat streaming only. One request → many event lines (text_chunk, tool_call,
//! done, etc.). Supports confirmation round-trips. Uses tokio for async execution.
//!
//! **Not this module**: For skill execution (run/exec/bash) and executor RPC (JSON-RPC 2.0,
//! one request → one response), see [`crate::stdio_rpc`]. That uses `skilllite serve --stdio`.
//!
//! This module belongs to the agent layer (Layer 3). It provides a transport-agnostic RPC
//! interface for Python/TypeScript SDKs to call the Rust agent engine.
//!
//! Protocol:
//!
//! Request (one JSON line on stdin):
//! ```json
//! {"method": "agent_chat", "params": {
//!     "message": "user input",
//!     "session_key": "default",
//!     "images": [ { "media_type": "image/png", "data_base64": "..." } ],
//!     "context": { "append": "optional string to append to system prompt" },
//!     "config": { "model": "gpt-4o", ... }  // optional overrides
//! }}
//! ```
//!
//! Response (multiple JSON lines on stdout):
//! ```json
//! {"event": "text_chunk", "data": {"text": "Hello"}}
//! {"event": "text", "data": {"text": "Hello, how can I help?"}}
//! {"event": "tool_call", "data": {"tool_call_id": "call_123", "name": "read_file", "arguments": "{...}"}}
//! {"event": "tool_result", "data": {"tool_call_id": "call_123", "name": "read_file", "result": "...", "is_error": false}}
//! {"event": "command_started", "data": {"command": "echo hello"}}
//! {"event": "command_output", "data": {"stream": "stdout", "chunk": "line"}}
//! {"event": "command_finished", "data": {"success": true, "exit_code": 0, "duration_ms": 123}}
//! {"event": "preview_started", "data": {"path": "dist", "port": 8765}}
//! {"event": "preview_ready", "data": {"url": "http://127.0.0.1:8765", "port": 8765}}
//! {"event": "preview_failed", "data": {"message": "port already in use"}}
//! {"event": "preview_stopped", "data": {"reason": "manual stop"}}
//! {"event": "swarm_started", "data": {"description": "delegate task"}}
//! {"event": "swarm_progress", "data": {"status": "submitting task"}}
//! {"event": "swarm_finished", "data": {"summary": "remote node completed task"}}
//! {"event": "swarm_failed", "data": {"message": "timeout, fallback to local execution"}}
//! {"event": "task_plan", "data": {"tasks": [...]}}
//! {"event": "task_progress", "data": {"task_id": 1, "completed": true}}
//! {"event": "llm_usage", "data": {"prompt_tokens": 1200, "completion_tokens": 80, "total_tokens": 1280}}
//! {"event": "llm_usage", "data": {"reported": false}}
//! {"event": "confirmation_request", "data": {"prompt": "Execute rm -rf?", "risk_tier": "confirm_required"}}
//! {"event": "clarification_request", "data": {"reason": "no_progress", "message": "...", "suggestions": ["...", "..."]}}
//! {"event": "done", "data": {"task_id": "...", "response": "...", "task_completed": true, "tool_calls": 3, "new_skill": null, "completion_type": "success", "llm_usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0, "responses_with_usage": 0, "responses_without_usage": 0}}}
//! {"event": "error", "data": {"message": "..."}}
//! ```
//!
//! For confirmation_request, the caller sends back:
//! ```json
//! {"method": "confirm", "params": {"approved": true}}
//! ```
//!
//! For clarification_request, the caller sends back:
//! ```json
//! {"method": "clarify", "params": {"action": "continue", "hint": "optional user input"}}
//! ```
//! or `{"method": "clarify", "params": {"action": "stop"}}`

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use skilllite_executor::transcript::{self, TranscriptEntry};

use super::types::*;
use super::{chat_session::ChatSession, skills};

// ─── RPC Event Sink ─────────────────────────────────────────────────────────

/// EventSink that writes JSON-Lines events to stdout.
/// Used by the agent_chat RPC protocol.
struct RpcEventSink {
    /// Shared writer for thread safety
    writer: Arc<Mutex<io::Stdout>>,
    /// Shared reader handle for confirmation prompts
    confirmation_rx: Arc<Mutex<BufReader<io::Stdin>>>,
    /// Current conversation turn index for dedupe scoping.
    turn_id: u64,
    /// Same-turn emitted tool_result keys to suppress duplicates in UI stream.
    emitted_tool_result_keys: HashSet<String>,
    /// True after `on_text_chunk` in this agent step; suppresses redundant full `on_text` (matches CLI TerminalEventSink).
    streamed_text: bool,
    /// When set, append resolved confirmation/clarification as `custom_message` for desktop reload.
    transcript_path: Option<PathBuf>,
}

impl RpcEventSink {
    fn new(
        writer: Arc<Mutex<io::Stdout>>,
        reader: Arc<Mutex<BufReader<io::Stdin>>>,
        transcript_path: Option<PathBuf>,
    ) -> Self {
        Self {
            writer,
            confirmation_rx: reader,
            turn_id: 0,
            emitted_tool_result_keys: HashSet::new(),
            streamed_text: false,
            transcript_path,
        }
    }

    fn emit(&self, event: &str, data: Value) {
        let msg = json!({ "event": event, "data": data });
        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(w, "{}", msg);
            let _ = w.flush();
        }
    }

    fn append_confirmation_transcript(&self, request: &ConfirmationRequest, approved: bool) {
        let Some(path) = &self.transcript_path else {
            return;
        };
        let entry = TranscriptEntry::CustomMessage {
            id: Uuid::new_v4().to_string(),
            parent_id: None,
            data: json!({
                "ui_kind": "confirmation",
                "prompt": request.prompt,
                "risk_tier": request.risk_tier,
                "resolved": true,
                "approved": approved,
            }),
        };
        if let Err(e) = transcript::append_entry(path, &entry) {
            tracing::warn!(
                target: "skilllite_agent_rpc",
                error = %e,
                "Failed to append confirmation to transcript"
            );
        }
    }

    fn append_clarification_transcript(
        &self,
        request: &ClarificationRequest,
        response: &ClarificationResponse,
    ) {
        let Some(path) = &self.transcript_path else {
            return;
        };
        let (action, hint) = match response {
            ClarificationResponse::Stop => ("stop", Value::Null),
            ClarificationResponse::Continue(h) => (
                "continue",
                h.as_ref().map(|s| json!(s)).unwrap_or(Value::Null),
            ),
        };
        let entry = TranscriptEntry::CustomMessage {
            id: Uuid::new_v4().to_string(),
            parent_id: None,
            data: json!({
                "ui_kind": "clarification",
                "reason": request.reason,
                "message": request.message,
                "suggestions": request.suggestions,
                "resolved": true,
                "action": action,
                "hint": hint,
            }),
        };
        if let Err(e) = transcript::append_entry(path, &entry) {
            tracing::warn!(
                target: "skilllite_agent_rpc",
                error = %e,
                "Failed to append clarification to transcript"
            );
        }
    }
}

fn build_tool_result_dedupe_key(turn_id: u64, name: &str, result: &str, is_error: bool) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    result.hash(&mut hasher);
    is_error.hash(&mut hasher);
    let content_hash = hasher.finish();
    format!("{turn_id}:{name}:{content_hash}:{is_error}")
}

fn build_tool_call_event_data(tool_call_id: Option<&str>, name: &str, arguments: &str) -> Value {
    let mut data = json!({ "name": name, "arguments": arguments });
    if let Some(id) = tool_call_id.filter(|id| !id.is_empty()) {
        data["tool_call_id"] = json!(id);
    }
    data
}

fn build_tool_result_event_data(
    tool_call_id: Option<&str>,
    name: &str,
    result: &str,
    is_error: bool,
) -> Value {
    let mut data = json!({ "name": name, "result": result, "is_error": is_error });
    if let Some(id) = tool_call_id.filter(|id| !id.is_empty()) {
        data["tool_call_id"] = json!(id);
    }
    data
}

impl EventSink for RpcEventSink {
    fn on_turn_start(&mut self) {
        self.turn_id = self.turn_id.saturating_add(1);
        self.emitted_tool_result_keys.clear();
        self.streamed_text = false;
    }

    fn reset_streamed_text_for_llm_call(&mut self) {
        self.streamed_text = false;
    }

    fn emit_assistant_visible(&mut self, text: &str) {
        // Tool-derived fallbacks / synthetic captions are not duplicates of the streamed LLM body.
        // After `text_chunk`, `on_text` intentionally drops one trailing full body to avoid echoing
        // the same completion twice — but `emit_assistant_visible` must still deliver new content
        // (e.g. substantive weather JSON) to the desktop UI.
        self.streamed_text = false;
        self.on_text(text);
    }

    fn on_text(&mut self, text: &str) {
        if self.streamed_text {
            // Already sent via `text_chunk` during streaming; avoid duplicate full `text`.
            // Agent loop uses `emit_assistant_visible` → this method, so reflection cannot double-send after chunks.
            self.streamed_text = false;
            return;
        }
        self.emit("text", json!({ "text": text }));
    }

    fn on_text_chunk(&mut self, chunk: &str) {
        self.streamed_text = true;
        self.emit("text_chunk", json!({ "text": chunk }));
    }

    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        self.emit("tool_call", json!({ "name": name, "arguments": arguments }));
    }

    fn on_tool_call_with_id(&mut self, tool_call_id: Option<&str>, name: &str, arguments: &str) {
        self.emit(
            "tool_call",
            build_tool_call_event_data(tool_call_id, name, arguments),
        );
    }

    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        let key = build_tool_result_dedupe_key(self.turn_id, name, result, is_error);
        if !self.emitted_tool_result_keys.insert(key) {
            return;
        }
        self.emit(
            "tool_result",
            json!({ "name": name, "result": result, "is_error": is_error }),
        );
    }

    fn on_tool_result_with_id(
        &mut self,
        tool_call_id: Option<&str>,
        name: &str,
        result: &str,
        is_error: bool,
    ) {
        let key = build_tool_result_dedupe_key(self.turn_id, name, result, is_error);
        if !self.emitted_tool_result_keys.insert(key) {
            return;
        }
        self.emit(
            "tool_result",
            build_tool_result_event_data(tool_call_id, name, result, is_error),
        );
    }

    fn on_command_started(&mut self, command: &str) {
        self.emit("command_started", json!({ "command": command }));
    }

    fn on_command_output(&mut self, stream: &str, chunk: &str) {
        self.emit(
            "command_output",
            json!({ "stream": stream, "chunk": chunk }),
        );
    }

    fn on_command_finished(&mut self, success: bool, exit_code: i32, duration_ms: u64) {
        self.emit(
            "command_finished",
            json!({ "success": success, "exit_code": exit_code, "duration_ms": duration_ms }),
        );
    }

    fn on_preview_started(&mut self, path: &str, port: u16) {
        self.emit("preview_started", json!({ "path": path, "port": port }));
    }

    fn on_preview_ready(&mut self, url: &str, port: u16) {
        self.emit("preview_ready", json!({ "url": url, "port": port }));
    }

    fn on_preview_failed(&mut self, message: &str) {
        self.emit("preview_failed", json!({ "message": message }));
    }

    fn on_preview_stopped(&mut self, reason: &str) {
        self.emit("preview_stopped", json!({ "reason": reason }));
    }

    fn on_swarm_started(&mut self, description: &str) {
        self.emit("swarm_started", json!({ "description": description }));
    }

    fn on_swarm_progress(&mut self, status: &str) {
        self.emit("swarm_progress", json!({ "status": status }));
    }

    fn on_swarm_finished(&mut self, summary: &str) {
        self.emit("swarm_finished", json!({ "summary": summary }));
    }

    fn on_swarm_failed(&mut self, message: &str) {
        self.emit("swarm_failed", json!({ "message": message }));
    }

    fn on_confirmation_request(&mut self, request: &ConfirmationRequest) -> bool {
        self.emit(
            "confirmation_request",
            json!({ "prompt": request.prompt, "risk_tier": request.risk_tier }),
        );

        if let Ok(mut reader) = self.confirmation_rx.lock() {
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                    if msg.get("method").and_then(|m| m.as_str()) == Some("confirm") {
                        let approved = msg
                            .get("params")
                            .and_then(|p| p.get("approved"))
                            .and_then(|a| a.as_bool())
                            .unwrap_or(false);
                        self.append_confirmation_transcript(request, approved);
                        return approved;
                    }
                }
            }
        }
        false
    }

    fn on_clarification_request(
        &mut self,
        request: &ClarificationRequest,
    ) -> ClarificationResponse {
        self.emit(
            "clarification_request",
            json!({
                "reason": request.reason,
                "message": request.message,
                "suggestions": request.suggestions,
            }),
        );

        if let Ok(mut reader) = self.confirmation_rx.lock() {
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                    if msg.get("method").and_then(|m| m.as_str()) == Some("clarify") {
                        let params = msg.get("params").cloned().unwrap_or(json!({}));
                        let action = params
                            .get("action")
                            .and_then(|a| a.as_str())
                            .unwrap_or("stop");
                        if action == "continue" {
                            let hint = params
                                .get("hint")
                                .and_then(|h| h.as_str())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string());
                            let response = ClarificationResponse::Continue(hint);
                            self.append_clarification_transcript(request, &response);
                            return response;
                        }
                    }
                }
            }
        }
        let response = ClarificationResponse::Stop;
        self.append_clarification_transcript(request, &response);
        response
    }

    fn on_task_plan(&mut self, tasks: &[Task]) {
        self.emit("task_plan", json!({ "tasks": tasks }));
    }

    fn on_task_progress(&mut self, task_id: u32, completed: bool, tasks: &[Task]) {
        self.emit(
            "task_progress",
            json!({ "task_id": task_id, "completed": completed, "tasks": tasks }),
        );
    }

    fn on_llm_usage(&mut self, usage: Option<LlmUsageReport>) {
        match usage {
            Some(u) => self.emit(
                "llm_usage",
                json!({
                    "prompt_tokens": u.prompt_tokens,
                    "completion_tokens": u.completion_tokens,
                    "total_tokens": u.total_tokens,
                }),
            ),
            None => self.emit("llm_usage", json!({ "reported": false })),
        }
    }
}

// ─── RPC Server ─────────────────────────────────────────────────────────────

/// Run the agent_chat RPC server over stdio.
///
/// Reads JSON-Lines from stdin, processes agent_chat requests,
/// streams events as JSON-Lines to stdout.
pub fn serve_agent_rpc() -> Result<()> {
    skilllite_core::config::ensure_default_output_dir();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let writer = Arc::new(Mutex::new(stdout));
    let reader_arc = Arc::new(Mutex::new(BufReader::new(stdin)));

    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    loop {
        let mut line = String::new();
        {
            let mut reader = reader_arc
                .lock()
                .map_err(|e| crate::Error::validation(format!("stdin lock poisoned: {}", e)))?;
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) => {
                    emit_event(
                        &writer,
                        "error",
                        json!({ "message": format!("stdin read error: {}", e) }),
                    );
                    break;
                }
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                emit_event(
                    &writer,
                    "error",
                    json!({ "message": format!("JSON parse error: {}", e) }),
                );
                continue;
            }
        };

        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        match method {
            "agent_chat" => {
                let writer_clone = Arc::clone(&writer);
                let reader_clone = Arc::clone(&reader_arc);
                if let Err(e) = rt.block_on(handle_agent_chat(&params, writer_clone, reader_clone))
                {
                    emit_event(&writer, "error", json!({ "message": e.to_string() }));
                }
            }
            "ping" => {
                emit_event(&writer, "pong", json!({}));
            }
            "confirm" | "clarify" => {
                // 进程管理端在 confirmation_request / clarification_request 后发送响应；
                // 若 agent_chat 已结束，主循环会读到滞后的消息。静默忽略。
            }
            _ => {
                emit_event(
                    &writer,
                    "error",
                    json!({ "message": format!("Unknown method: {}", method) }),
                );
            }
        }
    }

    Ok(())
}

fn emit_event(writer: &Arc<Mutex<io::Stdout>>, event: &str, data: Value) {
    let msg = json!({ "event": event, "data": data });
    if let Ok(mut w) = writer.lock() {
        let _ = writeln!(w, "{}", msg);
        let _ = w.flush();
    }
}

fn parse_agent_chat_images(
    params: &Value,
) -> Result<Option<Vec<crate::types::UserImageAttachment>>> {
    use crate::llm::normalize_vision_media_type;

    let Some(arr) = params.get("images").and_then(|v| v.as_array()) else {
        return Ok(None);
    };
    if arr.is_empty() {
        return Ok(None);
    }
    const MAX_IMAGES: usize = 6;
    const MAX_B64_CHARS: usize = 7_200_000;
    if arr.len() > MAX_IMAGES {
        bail!("At most {} images per message", MAX_IMAGES);
    }
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let media_type = item
            .get("media_type")
            .and_then(|v| v.as_str())
            .with_context(|| format!("images[{}].media_type (string) required", i))?;
        let canonical = normalize_vision_media_type(media_type)?;
        let data_base64 = item
            .get("data_base64")
            .and_then(|v| v.as_str())
            .with_context(|| format!("images[{}].data_base64 (string) required", i))?;
        let data_base64 = data_base64.trim();
        if data_base64.is_empty() {
            bail!("images[{}].data_base64 is empty", i);
        }
        if data_base64.len() > MAX_B64_CHARS {
            bail!("images[{}]: base64 payload too large", i);
        }
        out.push(crate::types::UserImageAttachment {
            media_type: canonical.to_string(),
            data_base64: data_base64.to_string(),
        });
    }
    Ok(Some(out))
}

async fn handle_agent_chat(
    params: &Value,
    writer: Arc<Mutex<io::Stdout>>,
    reader: Arc<Mutex<BufReader<io::Stdin>>>,
) -> Result<()> {
    let message = params
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    let images = parse_agent_chat_images(params)?;
    let has_images = images.as_ref().is_some_and(|v| !v.is_empty());
    if message.trim().is_empty() && !has_images {
        bail!("agent_chat requires non-empty 'message' and/or non-empty 'images'");
    }
    let session_key = params
        .get("session_key")
        .and_then(|s| s.as_str())
        .unwrap_or("default");

    let mut config = AgentConfig::from_env();
    if let Some(overrides) = params.get("config") {
        if let Some(model) = overrides.get("model").and_then(|v| v.as_str()) {
            config.model = model.to_string();
        }
        if let Some(base) = overrides.get("api_base").and_then(|v| v.as_str()) {
            config.api_base = base.to_string();
        }
        if let Some(key) = overrides.get("api_key").and_then(|v| v.as_str()) {
            config.api_key = key.to_string();
        }
        if let Some(ws) = overrides.get("workspace").and_then(|v| v.as_str()) {
            config.workspace = ws.to_string();
        }
        if let Some(max) = overrides.get("max_iterations").and_then(|v| v.as_u64()) {
            config.max_iterations = max as usize;
        }
        if let Some(n) = overrides
            .get("max_tool_calls_per_task")
            .and_then(|v| v.as_u64())
        {
            config.max_tool_calls_per_task = n as usize;
        }
        if let Some(plan) = overrides
            .get("enable_task_planning")
            .and_then(|v| v.as_bool())
        {
            config.enable_task_planning = plan;
        }
        if let Some(sp) = overrides.get("soul_path").and_then(|v| v.as_str()) {
            config.soul_path = Some(sp.to_string());
        }
        if let Some(skip) = overrides
            .get("skip_history_for_planning")
            .and_then(|v| v.as_bool())
        {
            config.skip_history_for_planning = skip;
        }
        if let Some(n) = overrides
            .get("max_consecutive_failures")
            .and_then(|v| v.as_u64())
        {
            config.max_consecutive_failures = if n == 0 { None } else { Some(n as usize) };
        }
    }
    // params.context.append — was documented but not parsed
    if let Some(ctx) = params
        .get("context")
        .and_then(|c| c.get("append"))
        .and_then(|a| a.as_str())
    {
        config.context_append = Some(ctx.to_string());
    }
    config.context_append =
        crate::locale_prompt::merge_ui_locale_env_into_context_append(config.context_append);

    if config.api_key.is_empty() {
        bail!("API key required. Set OPENAI_API_KEY env var.");
    }

    let skill_dirs: Vec<String> =
        if let Some(dirs) = params.get("skill_dirs").and_then(|v| v.as_array()) {
            dirs.iter()
                .filter_map(|d| d.as_str().map(|s| s.to_string()))
                .collect()
        } else {
            skilllite_core::skill::discovery::discover_skill_dirs_for_loading(
                Path::new(&config.workspace),
                Some(&[".skills", "skills"]),
            )
        };

    let loaded_skills = skills::load_skills(&skill_dirs);

    let mut session = ChatSession::new(config, session_key, loaded_skills);
    let transcript_path = session.transcript_append_path();
    let mut sink = RpcEventSink::new(writer.clone(), reader, Some(transcript_path));

    match session
        .run_turn_with_media(&message, images, &mut sink)
        .await
    {
        Ok(agent_result) => {
            let task_id = Uuid::new_v4().to_string();
            let node_result = agent_result.to_node_result(&task_id);
            let mut data = serde_json::to_value(&node_result).unwrap_or_else(|_| {
                serde_json::json!({
                    "task_id": task_id,
                    "response": agent_result.response,
                    "task_completed": agent_result.feedback.task_completed,
                    "tool_calls": agent_result.feedback.total_tools,
                    "new_skill": serde_json::Value::Null
                })
            });
            if let Some(obj) = data.as_object_mut() {
                obj.insert(
                    "completion_type".to_string(),
                    serde_json::Value::String(
                        agent_result.feedback.completion_type.as_str().to_string(),
                    ),
                );
                obj.insert(
                    "llm_usage".to_string(),
                    serde_json::to_value(agent_result.feedback.llm_usage).unwrap_or(json!({})),
                );
            }
            emit_event(&writer, "done", data);
        }
        Err(e) => {
            emit_event(&writer, "error", json!({ "message": e.to_string() }));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tool_call_event_data, build_tool_result_dedupe_key, build_tool_result_event_data,
    };

    #[test]
    fn dedupe_key_is_stable_for_same_input() {
        let a = build_tool_result_dedupe_key(1, "weather", "ok", false);
        let b = build_tool_result_dedupe_key(1, "weather", "ok", false);
        assert_eq!(a, b);
    }

    #[test]
    fn dedupe_key_changes_on_turn_or_content_or_error_flag() {
        let base = build_tool_result_dedupe_key(1, "weather", "ok", false);
        let different_turn = build_tool_result_dedupe_key(2, "weather", "ok", false);
        let different_content = build_tool_result_dedupe_key(1, "weather", "ok2", false);
        let different_error = build_tool_result_dedupe_key(1, "weather", "ok", true);
        assert_ne!(base, different_turn);
        assert_ne!(base, different_content);
        assert_ne!(base, different_error);
    }

    #[test]
    fn tool_event_payloads_include_tool_call_id_when_present() {
        let call = build_tool_call_event_data(Some("call-123"), "read_file", "{\"path\":\"a\"}");
        let result = build_tool_result_event_data(Some("call-123"), "read_file", "ok", false);
        assert_eq!(call["tool_call_id"], "call-123");
        assert_eq!(result["tool_call_id"], "call-123");
    }

    #[test]
    fn tool_event_payloads_omit_empty_tool_call_id() {
        let call = build_tool_call_event_data(Some(""), "read_file", "{}");
        let result = build_tool_result_event_data(None, "read_file", "ok", false);
        assert!(call.get("tool_call_id").is_none());
        assert!(result.get("tool_call_id").is_none());
    }
}
