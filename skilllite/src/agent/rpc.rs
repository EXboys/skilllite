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
//!     "config": { "model": "gpt-4o", ... }  // optional overrides
//! }}
//! ```
//!
//! Response (multiple JSON lines on stdout):
//! ```json
//! {"event": "text_chunk", "data": {"text": "Hello"}}
//! {"event": "text", "data": {"text": "Hello, how can I help?"}}
//! {"event": "tool_call", "data": {"name": "read_file", "arguments": "{...}"}}
//! {"event": "tool_result", "data": {"name": "read_file", "result": "...", "is_error": false}}
//! {"event": "task_plan", "data": {"tasks": [...]}}
//! {"event": "task_progress", "data": {"task_id": 1, "completed": true}}
//! {"event": "confirmation_request", "data": {"prompt": "Execute rm -rf?"}}
//! {"event": "done", "data": {"response": "...", "tool_calls_count": 3, "iterations": 2}}
//! {"event": "error", "data": {"message": "..."}}
//! ```
//!
//! For confirmation_request, the caller sends back:
//! ```json
//! {"method": "confirm", "params": {"approved": true}}
//! ```

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
}

impl RpcEventSink {
    fn new(
        writer: Arc<Mutex<io::Stdout>>,
        reader: Arc<Mutex<BufReader<io::Stdin>>>,
    ) -> Self {
        Self {
            writer,
            confirmation_rx: reader,
        }
    }

    fn emit(&self, event: &str, data: Value) {
        let msg = json!({ "event": event, "data": data });
        if let Ok(mut w) = self.writer.lock() {
            let _ = writeln!(w, "{}", msg);
            let _ = w.flush();
        }
    }
}

impl EventSink for RpcEventSink {
    fn on_text(&mut self, text: &str) {
        self.emit("text", json!({ "text": text }));
    }

    fn on_text_chunk(&mut self, chunk: &str) {
        self.emit("text_chunk", json!({ "text": chunk }));
    }

    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        self.emit(
            "tool_call",
            json!({ "name": name, "arguments": arguments }),
        );
    }

    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        self.emit(
            "tool_result",
            json!({ "name": name, "result": result, "is_error": is_error }),
        );
    }

    fn on_confirmation_request(&mut self, prompt: &str) -> bool {
        self.emit("confirmation_request", json!({ "prompt": prompt }));

        if let Ok(mut reader) = self.confirmation_rx.lock() {
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
                    if msg.get("method").and_then(|m| m.as_str()) == Some("confirm") {
                        return msg
                            .get("params")
                            .and_then(|p| p.get("approved"))
                            .and_then(|a| a.as_bool())
                            .unwrap_or(false);
                    }
                }
            }
        }
        false
    }

    fn on_task_plan(&mut self, tasks: &[Task]) {
        self.emit("task_plan", json!({ "tasks": tasks }));
    }

    fn on_task_progress(&mut self, task_id: u32, completed: bool) {
        self.emit(
            "task_progress",
            json!({ "task_id": task_id, "completed": completed }),
        );
    }
}

// ─── RPC Server ─────────────────────────────────────────────────────────────

/// Run the agent_chat RPC server over stdio.
///
/// Reads JSON-Lines from stdin, processes agent_chat requests,
/// streams events as JSON-Lines to stdout.
pub fn serve_agent_rpc() -> Result<()> {
    // Set default output directory BEFORE creating the tokio runtime (multi-threaded).
    // SAFETY: No other threads exist at this point.
    if std::env::var("SKILLLITE_OUTPUT_DIR").is_err() {
        let chat_output = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output");
        unsafe { std::env::set_var("SKILLLITE_OUTPUT_DIR", chat_output.to_string_lossy().as_ref()) };
    }
    if let Ok(output_dir) = std::env::var("SKILLLITE_OUTPUT_DIR") {
        let p = PathBuf::from(&output_dir);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let writer = Arc::new(Mutex::new(stdout));
    let reader_arc = Arc::new(Mutex::new(BufReader::new(stdin)));

    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    loop {
        let mut line = String::new();
        {
            let mut reader = reader_arc.lock().unwrap();
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

        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(json!({}));

        match method {
            "agent_chat" => {
                let writer_clone = Arc::clone(&writer);
                let reader_clone = Arc::clone(&reader_arc);
                if let Err(e) = rt.block_on(handle_agent_chat(
                    &params,
                    writer_clone,
                    reader_clone,
                )) {
                    emit_event(
                        &writer,
                        "error",
                        json!({ "message": e.to_string() }),
                    );
                }
            }
            "ping" => {
                emit_event(&writer, "pong", json!({}));
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

async fn handle_agent_chat(
    params: &Value,
    writer: Arc<Mutex<io::Stdout>>,
    reader: Arc<Mutex<BufReader<io::Stdin>>>,
) -> Result<()> {
    let message = params
        .get("message")
        .and_then(|m| m.as_str())
        .context("'message' is required in agent_chat params")?;
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
        if let Some(plan) = overrides.get("enable_task_planning").and_then(|v| v.as_bool()) {
            config.enable_task_planning = plan;
        }
    }

    if config.api_key.is_empty() {
        anyhow::bail!("API key required. Set OPENAI_API_KEY env var.");
    }

    let skill_dirs: Vec<String> = if let Some(dirs) = params.get("skill_dirs").and_then(|v| v.as_array()) {
        dirs.iter()
            .filter_map(|d| d.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        auto_discover_skill_dirs(&config.workspace)
    };

    let loaded_skills = skills::load_skills(&skill_dirs);

    let mut session = ChatSession::new(config, session_key, loaded_skills);
    let mut sink = RpcEventSink::new(writer.clone(), reader);

    match session.run_turn(message, &mut sink).await {
        Ok(response) => {
            emit_event(
                &writer,
                "done",
                json!({ "response": response }),
            );
        }
        Err(e) => {
            emit_event(
                &writer,
                "error",
                json!({ "message": e.to_string() }),
            );
        }
    }

    Ok(())
}

fn auto_discover_skill_dirs(workspace: &str) -> Vec<String> {
    let ws = Path::new(workspace);
    let mut dirs = Vec::new();
    for name in &[".skills", "skills"] {
        let dir = ws.join(name);
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("SKILL.md").exists() {
                        dirs.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    dirs
}
