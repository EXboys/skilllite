//! 与 `skilllite agent-rpc` 子进程交互：启动、JSON-RPC、确认/澄清、事件转发。

use serde::Serialize;
use serde_json::{json, Value};
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
    make_protocol_recovered_event, make_protocol_warning_event, parse_stream_event_line,
    preview_line, StreamEvent, INVALID_LINE_PREVIEW_CHARS, MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES,
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

/// Single image from the desktop UI (vision); sent to `agent_chat` as base64.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ChatImageAttachment {
    pub media_type: String,
    pub data_base64: String,
}

/// Config overrides from frontend (optional).
#[derive(Clone, Debug, serde::Deserialize, Default)]
pub struct ChatConfigOverrides {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub workspace: Option<String>,
    pub sandbox_level: Option<u8>,
    pub swarm_url: Option<String>,
    pub max_iterations: Option<u32>,
    pub max_tool_calls_per_task: Option<u32>,
    /// 覆盖 `SKILLLITE_CONTEXT_SOFT_LIMIT_CHARS`（预请求上下文软上限；`0` 表示关闭收缩）。
    pub context_soft_limit_chars: Option<u32>,
    /// 覆盖 `SKILLLITE_EVOLUTION_INTERVAL_SECS`（Life Pulse / 状态展示）。
    pub evolution_interval_secs: Option<u64>,
    /// 覆盖 `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`。
    pub evolution_decision_threshold: Option<u32>,
    /// 覆盖 `SKILLLITE_EVO_PROFILE`：`demo` / `conservative`；不设则沿用工作区合并后的环境配置。
    pub evo_profile: Option<String>,
    /// 覆盖 `SKILLLITE_EVO_COOLDOWN_HOURS`（被动提案冷却）。
    pub evo_cooldown_hours: Option<f64>,
    /// 桌面界面语言：`zh` | `en` → 写入子进程 `SKILLLITE_UI_LOCALE`，与聊天 / 定时任务共用。
    pub ui_locale: Option<String>,
}

/// Merge workspace dotenv-derived pairs with optional UI overrides (same semantics as [`chat_stream`]).
/// When `overrides` is `None`, returns `dotenv` unchanged (backward compatible).
pub fn merge_dotenv_with_chat_overrides(
    dotenv: Vec<(String, String)>,
    overrides: Option<&ChatConfigOverrides>,
) -> Vec<(String, String)> {
    let Some(cfg) = overrides else {
        return dotenv;
    };

    use std::collections::HashMap;
    let mut m: HashMap<String, String> = dotenv.into_iter().collect();

    let swarm_from_ui = cfg
        .swarm_url
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if !swarm_from_ui {
        m.remove("SKILLLITE_SWARM_URL");
    }

    if let Some(ref key) = cfg.api_key {
        if !key.is_empty() {
            m.insert("OPENAI_API_KEY".to_string(), key.clone());
        }
    }
    if let Some(ref base) = cfg.api_base {
        if !base.is_empty() {
            m.insert("OPENAI_BASE_URL".to_string(), base.clone());
        }
    }
    if let Some(ref model) = cfg.model {
        if !model.is_empty() {
            m.insert("OPENAI_MODEL".to_string(), model.clone());
            m.insert(
                skilllite_core::config::env_keys::llm::MODEL.to_string(),
                model.clone(),
            );
        }
    }
    if let Some(level) = cfg.sandbox_level {
        if (1..=3).contains(&level) {
            m.insert("SKILLLITE_SANDBOX_LEVEL".to_string(), level.to_string());
        }
    }
    if let Some(ref url) = cfg.swarm_url {
        if !url.is_empty() {
            m.insert("SKILLLITE_SWARM_URL".to_string(), url.clone());
        }
    }
    if let Some(n) = cfg.max_iterations.filter(|&n| n > 0) {
        m.insert("SKILLLITE_MAX_ITERATIONS".to_string(), n.to_string());
    }
    if let Some(n) = cfg.max_tool_calls_per_task.filter(|&n| n > 0) {
        m.insert(
            "SKILLLITE_MAX_TOOL_CALLS_PER_TASK".to_string(),
            n.to_string(),
        );
    }
    if let Some(n) = cfg.context_soft_limit_chars {
        m.insert(
            "SKILLLITE_CONTEXT_SOFT_LIMIT_CHARS".to_string(),
            n.to_string(),
        );
    }

    use skilllite_core::config::env_keys::evolution as evo_keys;
    if let Some(n) = cfg.evolution_interval_secs.filter(|&n| n > 0) {
        m.insert(
            evo_keys::SKILLLITE_EVOLUTION_INTERVAL_SECS.to_string(),
            n.to_string(),
        );
    }
    if let Some(n) = cfg.evolution_decision_threshold.filter(|&n| n > 0) {
        m.insert(
            evo_keys::SKILLLITE_EVOLUTION_DECISION_THRESHOLD.to_string(),
            n.to_string(),
        );
    }
    if let Some(ref p) = cfg.evo_profile {
        let t = p.trim();
        if t == "demo" || t == "conservative" {
            m.insert(evo_keys::SKILLLITE_EVO_PROFILE.to_string(), t.to_string());
        }
    }
    if let Some(h) = cfg
        .evo_cooldown_hours
        .filter(|h| h.is_finite() && *h >= 0.0)
    {
        m.insert(
            evo_keys::SKILLLITE_EVO_COOLDOWN_HOURS.to_string(),
            format!("{h}"),
        );
    }

    use skilllite_core::config::env_keys::agent as agent_keys;
    if let Some(ref loc) = cfg.ui_locale {
        let t = loc.trim();
        if t == "en" || t == "zh" {
            m.insert(agent_keys::SKILLLITE_UI_LOCALE.to_string(), t.to_string());
        }
    }

    m.into_iter().collect()
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
/// The parameter list mirrors the Tauri command boundary and shared state inputs.
#[allow(clippy::too_many_arguments)]
pub fn chat_stream(
    window: Window,
    message: String,
    workspace: Option<String>,
    config_overrides: Option<ChatConfigOverrides>,
    session_key: Option<String>,
    images: Option<Vec<ChatImageAttachment>>,
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
        bundled_skills_sync::sync_bundled_skills_from_resources(window.app_handle(), &raw_workspace)
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

    let env_pairs = merge_dotenv_with_chat_overrides(
        load_dotenv_for_child(&raw_workspace),
        config_overrides.as_ref(),
    );
    for (k, v) in env_pairs {
        cmd.env(k, v);
    }
    cmd.env("RUST_LOG", "error");
    cmd.env("SKILLLITE_QUIET", "1");
    cmd.env("SKILLLITE_LOG_JSON", "0");

    crate::windows_spawn::hide_child_console(&mut cmd);

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
        if let Some(n) = cfg.context_soft_limit_chars {
            config_json.insert("context_soft_limit_chars".to_string(), json!(n));
        }
    }

    let session = session_key.unwrap_or_else(|| "default".to_string());
    let imgs_json: Option<Vec<Value>> = images.map(|v| {
        v.into_iter()
            .map(|img| {
                json!({
                    "media_type": img.media_type,
                    "data_base64": img.data_base64,
                })
            })
            .collect()
    });
    let mut params = json!({
        "message": message,
        "session_key": session.clone(),
        "config": config_json
    });
    if let Some(obj) = params.as_object_mut() {
        if let Some(arr) = imgs_json.filter(|a| !a.is_empty()) {
            obj.insert("images".to_string(), json!(arr));
        }
    }
    let request = json!({
        "method": "agent_chat",
        "params": params
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
                    let risk_tier = ev
                        .data
                        .get("risk_tier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("confirm_required");
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
                        json!({ "prompt": prompt, "risk_tier": risk_tier, "session_key": &session }),
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
                    let message = ev
                        .data
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
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
