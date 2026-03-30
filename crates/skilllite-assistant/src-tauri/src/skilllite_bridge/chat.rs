//! 与 `skilllite agent-rpc` 子进程交互：启动、JSON-RPC、确认/澄清、事件转发。

use serde::Serialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager, Window};

use super::bundled_skills_sync;
use super::paths::{find_project_root, load_dotenv_for_child};
use super::protocol::{
    make_protocol_recovered_event, make_protocol_warning_event, parse_stream_event_line, preview_line,
    StreamEvent, INVALID_LINE_PREVIEW_CHARS, MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES,
    MAX_TOTAL_INVALID_PROTOCOL_LINES,
};

/// Shared state for confirmation flow: frontend calls skilllite_confirm → sends to this channel.
#[derive(Default, Clone)]
pub struct ConfirmationState(pub Arc<Mutex<Option<mpsc::Sender<bool>>>>);

/// Response payload for clarification flow.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ClarifyResponse {
    pub action: String,
    pub hint: Option<String>,
}

/// Shared state for clarification flow: frontend calls skilllite_clarify → sends to this channel.
#[derive(Default, Clone)]
pub struct ClarificationState(pub Arc<Mutex<Option<mpsc::Sender<ClarifyResponse>>>>);

/// Shared state for the chat subprocess; skilllite_stop can kill it.
#[derive(Default, Clone)]
pub struct ChatProcessState(pub Arc<Mutex<Option<std::process::Child>>>);

/// Config overrides from frontend (optional).
#[derive(serde::Deserialize, Default)]
pub struct ChatConfigOverrides {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub workspace: Option<String>,
    pub sandbox_level: Option<u8>,
    pub swarm_url: Option<String>,
    pub max_iterations: Option<u32>,
    pub max_tool_calls_per_task: Option<u32>,
}

fn resolve_skilllite_path(window: &Window) -> (PathBuf, bool) {
    let exe_name = if cfg!(target_os = "windows") {
        "skilllite.exe"
    } else {
        "skilllite"
    };

    #[cfg(debug_assertions)]
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            return (dev_bin, true);
        }
    }

    if let Ok(res_dir) = window.app_handle().path().resource_dir() {
        let bundled = res_dir.join(exe_name);
        if bundled.exists() {
            return (bundled, true);
        }
    }

    #[cfg(not(debug_assertions))]
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            return (dev_bin, true);
        }
    }

    (PathBuf::from("skilllite"), false)
}

/// Run agent_chat via skilllite agent-rpc subprocess, emitting events to the window.
pub fn chat_stream(
    window: Window,
    message: String,
    workspace: Option<String>,
    config_overrides: Option<ChatConfigOverrides>,
    session_key: Option<String>,
    confirmation_state: ConfirmationState,
    clarification_state: ClarificationState,
    process_state: ChatProcessState,
) -> Result<(), String> {
    let raw_workspace = workspace
        .or_else(|| config_overrides.as_ref().and_then(|c| c.workspace.clone()))
        .unwrap_or_else(|| ".".to_string());
    let workspace_root = find_project_root(&raw_workspace);
    let workspace_str = workspace_root.to_string_lossy().to_string();

    if let Err(e) =
        bundled_skills_sync::sync_bundled_skills_from_resources(&window.app_handle(), &raw_workspace)
    {
        eprintln!("[skilllite-assistant] bundled skills sync failed: {}", e);
    }

    let (skilllite_path, is_bundled) = resolve_skilllite_path(&window);
    let mut cmd = Command::new(&skilllite_path);
    cmd.args(["agent-rpc"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(&workspace_root);

    for (k, v) in load_dotenv_for_child(&raw_workspace) {
        cmd.env(k, v);
    }
    // Assistant passes `config` on every chat: Swarm is UI-controlled. If there is no non-empty
    // `swarm_url` override, do not inherit SKILLLITE_SWARM_URL from .env (otherwise "Swarm off"
    // in settings still delegates).
    if let Some(ref cfg) = config_overrides {
        let swarm_from_ui = cfg
            .swarm_url
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if !swarm_from_ui {
            cmd.env_remove("SKILLLITE_SWARM_URL");
        }
    }
    cmd.env("RUST_LOG", "error");
    cmd.env("SKILLLITE_QUIET", "1");
    cmd.env("SKILLLITE_LOG_JSON", "0");
    if let Some(ref cfg) = config_overrides {
        if let Some(ref key) = cfg.api_key {
            if !key.is_empty() {
                cmd.env("OPENAI_API_KEY", key);
            }
        }
        if let Some(ref base) = cfg.api_base {
            if !base.is_empty() {
                cmd.env("OPENAI_BASE_URL", base);
            }
        }
        if let Some(level) = cfg.sandbox_level {
            if (1..=3).contains(&level) {
                cmd.env("SKILLLITE_SANDBOX_LEVEL", level.to_string());
            }
        }
        if let Some(ref url) = cfg.swarm_url {
            if !url.is_empty() {
                cmd.env("SKILLLITE_SWARM_URL", url);
            }
        }
        if let Some(n) = cfg.max_iterations.filter(|&n| n > 0) {
            cmd.env("SKILLLITE_MAX_ITERATIONS", n.to_string());
        }
        if let Some(n) = cfg.max_tool_calls_per_task.filter(|&n| n > 0) {
            cmd.env("SKILLLITE_MAX_TOOL_CALLS_PER_TASK", n.to_string());
        }
    }

    let mut child = cmd.spawn().map_err(|e| {
        if is_bundled {
            format!("Failed to spawn bundled skilllite: {}", e)
        } else {
            format!(
                "Failed to spawn skilllite: {}. Ensure skilllite is installed and in PATH: cargo install --path skilllite --features memory_vector (or add ~/.skilllite/bin to PATH)",
                e
            )
        }
    })?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open stdin".to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open stdout".to_string())?;

    {
        let mut guard = process_state
            .0
            .lock()
            .map_err(|_| "ChatProcessState lock poisoned")?;
        *guard = Some(child);
    }

    let mut config_json = serde_json::Map::new();
    config_json.insert("workspace".to_string(), json!(workspace_str));
    if let Some(ref cfg) = config_overrides {
        if let Some(ref k) = cfg.api_key {
            if !k.is_empty() {
                config_json.insert("api_key".to_string(), json!(k));
            }
        }
        if let Some(ref m) = cfg.model {
            if !m.is_empty() {
                config_json.insert("model".to_string(), json!(m));
            }
        }
        if let Some(ref b) = cfg.api_base {
            if !b.is_empty() {
                config_json.insert("api_base".to_string(), json!(b));
            }
        }
        if let Some(level) = cfg.sandbox_level {
            if (1..=3).contains(&level) {
                config_json.insert("sandbox_level".to_string(), json!(level));
            }
        }
        if let Some(ref url) = cfg.swarm_url {
            if !url.is_empty() {
                config_json.insert("swarm_url".to_string(), json!(url));
            }
        }
        if let Some(n) = cfg.max_iterations.filter(|&n| n > 0) {
            config_json.insert("max_iterations".to_string(), json!(n));
        }
        if let Some(n) = cfg.max_tool_calls_per_task.filter(|&n| n > 0) {
            config_json.insert("max_tool_calls_per_task".to_string(), json!(n));
        }
    }

    let session = session_key.unwrap_or_else(|| "default".to_string());
    let request = json!({
        "method": "agent_chat",
        "params": {
            "message": message,
            "session_key": session.clone(),
            "config": config_json
        }
    });
    writeln!(stdin, "{}", request).map_err(|e| e.to_string())?;

    let (tx, rx) = mpsc::channel::<Result<StreamEvent, String>>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut consecutive_invalid_lines = 0usize;
        let mut total_invalid_lines = 0usize;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                    break;
                }
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match parse_stream_event_line(line) {
                Ok(event) => {
                    if consecutive_invalid_lines > 0 {
                        let recovered = make_protocol_recovered_event(
                            consecutive_invalid_lines,
                            total_invalid_lines,
                        );
                        if tx.send(Ok(recovered)).is_err() {
                            break;
                        }
                    }
                    consecutive_invalid_lines = 0;
                    if tx.send(Ok(event)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    consecutive_invalid_lines += 1;
                    total_invalid_lines += 1;
                    if consecutive_invalid_lines == 1 {
                        let warning = make_protocol_warning_event(
                            consecutive_invalid_lines,
                            total_invalid_lines,
                            line,
                            &err,
                        );
                        if tx.send(Ok(warning)).is_err() {
                            break;
                        }
                    }
                    eprintln!(
                        "[skilllite-bridge] Ignoring invalid agent-rpc line ({}/{} consecutive, {} total): {} | raw line: {:?}",
                        consecutive_invalid_lines,
                        MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES,
                        total_invalid_lines,
                        err,
                        preview_line(line, INVALID_LINE_PREVIEW_CHARS)
                    );

                    if consecutive_invalid_lines >= MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES
                        || total_invalid_lines >= MAX_TOTAL_INVALID_PROTOCOL_LINES
                    {
                        let _ = tx.send(Err(format!(
                            "agent-rpc protocol stream corrupted after {} invalid line(s) ({} consecutive). Last error: {}",
                            total_invalid_lines, consecutive_invalid_lines, err
                        )));
                        break;
                    }
                }
            }
        }
    });

    #[derive(Serialize)]
    struct TaggedEvent<'a> {
        event: &'a str,
        data: &'a serde_json::Value,
        session_key: &'a str,
    }

    for msg in rx {
        match msg {
            Ok(ev) => {
                if ev.event == "confirmation_request" {
                    let prompt = ev.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                    let (confirm_tx, confirm_rx) = mpsc::channel();
                    {
                        let mut guard = confirmation_state
                            .0
                            .lock()
                            .map_err(|_| "ConfirmationState lock poisoned")?;
                        *guard = Some(confirm_tx);
                    }
                    if let Err(e) = window.emit(
                        "skilllite-confirmation-request",
                        json!({ "prompt": prompt, "session_key": &session }),
                    ) {
                        eprintln!("emit confirmation_request error: {}", e);
                    }
                    let approved = confirm_rx.recv().unwrap_or(false);
                    {
                        let mut guard = confirmation_state
                            .0
                            .lock()
                            .map_err(|_| "ConfirmationState lock poisoned")?;
                        *guard = None;
                    }
                    let confirm_msg =
                        json!({ "method": "confirm", "params": { "approved": approved } });
                    if let Err(e) = writeln!(stdin, "{}", confirm_msg) {
                        eprintln!("write confirm error: {}", e);
                    }
                    let _ = stdin.flush();
                    continue;
                }
                if ev.event == "clarification_request" {
                    let reason = ev.data.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                    let message = ev.data.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    let suggestions: Vec<String> = ev
                        .data
                        .get("suggestions")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let (clarify_tx, clarify_rx) = mpsc::channel();
                    {
                        let mut guard = clarification_state
                            .0
                            .lock()
                            .map_err(|_| "ClarificationState lock poisoned")?;
                        *guard = Some(clarify_tx);
                    }
                    if let Err(e) = window.emit(
                        "skilllite-clarification-request",
                        json!({
                            "reason": reason,
                            "message": message,
                            "suggestions": suggestions,
                            "session_key": &session,
                        }),
                    ) {
                        eprintln!("emit clarification_request error: {}", e);
                    }
                    let response = clarify_rx.recv().unwrap_or(ClarifyResponse {
                        action: "stop".into(),
                        hint: None,
                    });
                    {
                        let mut guard = clarification_state
                            .0
                            .lock()
                            .map_err(|_| "ClarificationState lock poisoned")?;
                        *guard = None;
                    }
                    let clarify_msg = json!({
                        "method": "clarify",
                        "params": {
                            "action": response.action,
                            "hint": response.hint.unwrap_or_default(),
                        }
                    });
                    if let Err(e) = writeln!(stdin, "{}", clarify_msg) {
                        eprintln!("write clarify error: {}", e);
                    }
                    let _ = stdin.flush();
                    continue;
                }
                if let Err(e) = window.emit(
                    "skilllite-event",
                    &TaggedEvent {
                        event: &ev.event,
                        data: &ev.data,
                        session_key: &session,
                    },
                ) {
                    eprintln!("emit error: {}", e);
                }
                if ev.event == "done" || ev.event == "error" {
                    break;
                }
            }
            Err(e) => {
                let err_data = json!({ "message": e });
                let _ = window.emit(
                    "skilllite-event",
                    &TaggedEvent {
                        event: "error",
                        data: &err_data,
                        session_key: &session,
                    },
                );
                break;
            }
        }
    }

    drop(stdin);
    let child_opt = {
        let mut guard = process_state
            .0
            .lock()
            .map_err(|_| "ChatProcessState lock poisoned")?;
        guard.take()
    };
    if let Some(mut c) = child_opt {
        let _ = c.wait();
    }
    Ok(())
}

pub fn stop_chat(
    process_state: &ChatProcessState,
    confirmation_state: &ConfirmationState,
    clarification_state: &ClarificationState,
) -> Result<(), String> {
    {
        let mut guard = confirmation_state
            .0
            .lock()
            .map_err(|_| "ConfirmationState lock poisoned")?;
        drop(guard.take());
    }
    {
        let mut guard = clarification_state
            .0
            .lock()
            .map_err(|_| "ClarificationState lock poisoned")?;
        drop(guard.take());
    }
    let mut guard = process_state
        .0
        .lock()
        .map_err(|_| "ChatProcessState lock poisoned")?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_chat_succeeds_when_no_child() {
        stop_chat(
            &ChatProcessState::default(),
            &ConfirmationState::default(),
            &ClarificationState::default(),
        )
        .unwrap();
    }
}
