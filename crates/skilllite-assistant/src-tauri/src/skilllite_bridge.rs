//! SkillLite bridge: spawn `skilllite agent-rpc` subprocess and forward JSON-RPC over stdio.
//!
//! Protocol: see crates/skilllite-agent/src/rpc.rs
//! Request: {"method":"agent_chat","params":{"message":"...","session_key":"default","config":{"workspace":"..."}}}
//! Response: JSON-Lines events (text_chunk, text, done, error, confirmation_request, etc.)
//!
//! For confirmation_request: emit to frontend, wait for skilllite_confirm, write to stdin.

use base64::Engine;
use serde::Serialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager, Window};

const MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES: usize = 8;
const MAX_TOTAL_INVALID_PROTOCOL_LINES: usize = 20;
const INVALID_LINE_PREVIEW_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub event: String,
    pub data: serde_json::Value,
}

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

/// Find project root (dir containing .skills or skills) by walking up from start path.
fn find_project_root(start: &str) -> std::path::PathBuf {
    use std::path::Path;
    let mut dir = Path::new(start)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(start).to_path_buf());
    for _ in 0..10 {
        if dir.join(".skills").is_dir() || dir.join("skills").is_dir() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    Path::new(start)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(start).to_path_buf())
}

/// Load .env from workspace and parents for subprocess env.
/// Reuses skilllite_core::config::parse_dotenv_walking_up (single source of truth).
fn load_dotenv_for_child(workspace: &str) -> Vec<(String, String)> {
    skilllite_core::config::parse_dotenv_walking_up(std::path::Path::new(workspace), 5)
}

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

/// Resolve skilllite binary path: bundled resource first, then ~/.skilllite/bin, else PATH.
fn resolve_skilllite_path(window: &Window) -> (PathBuf, bool) {
    let exe_name = if cfg!(target_os = "windows") {
        "skilllite.exe"
    } else {
        "skilllite"
    };
    if let Ok(res_dir) = window.app_handle().path().resource_dir() {
        let bundled = res_dir.join(exe_name);
        if bundled.exists() {
            return (bundled, true);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            return (dev_bin, true);
        }
    }
    (PathBuf::from("skilllite"), false)
}

fn preview_line(line: &str, max_chars: usize) -> String {
    let preview: String = line.chars().take(max_chars).collect();
    if line.chars().count() > max_chars {
        format!("{}...", preview)
    } else {
        preview
    }
}

fn parse_stream_event_line(line: &str) -> Result<StreamEvent, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;
    let event = value
        .get("event")
        .and_then(|e| e.as_str())
        .ok_or_else(|| "Protocol error: missing string field 'event'".to_string())?;
    let data = value
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(StreamEvent {
        event: event.to_string(),
        data,
    })
}

fn make_protocol_warning_event(
    consecutive_invalid_lines: usize,
    total_invalid_lines: usize,
    line: &str,
    err: &str,
) -> StreamEvent {
    StreamEvent {
        event: "protocol_warning".to_string(),
        data: json!({
            "message": "检测到 agent-rpc 协议流被脏数据污染，正在自动恢复",
            "consecutive_invalid_lines": consecutive_invalid_lines,
            "total_invalid_lines": total_invalid_lines,
            "line_preview": preview_line(line, INVALID_LINE_PREVIEW_CHARS),
            "last_error": err,
        }),
    }
}

fn make_protocol_recovered_event(
    recovered_lines: usize,
    total_invalid_lines: usize,
) -> StreamEvent {
    StreamEvent {
        event: "protocol_recovered".to_string(),
        data: json!({
            "message": format!(
                "agent-rpc 协议流已自动恢复，已跳过 {} 行异常输出",
                recovered_lines
            ),
            "recovered_lines": recovered_lines,
            "total_invalid_lines": total_invalid_lines,
        }),
    }
}

/// Run agent_chat via skilllite agent-rpc subprocess, emitting events to the window.
/// Loads .env from workspace (or parent dirs) so OPENAI_API_KEY etc. are available.
/// Config overrides (api_key, model, etc.) are merged into the request.
/// For confirmation_request / clarification_request: emits to frontend, waits for response, writes to stdin.
/// Uses bundled skilllite if available (single-file distribution), else PATH.
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
    // Override logging: prevent tracing etc. from polluting stdout (JSON-Lines protocol).
    // Critical for "other computer" / distribution builds where env may differ.
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
            config_json.insert(
                "max_tool_calls_per_task".to_string(),
                json!(n),
            );
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
    // Keep stdin open for confirmation responses

    // Read stdout in a thread and emit events
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

    // Process events on main thread and emit to window
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

#[cfg(test)]
mod tests {
    use super::{
        make_protocol_recovered_event, make_protocol_warning_event, parse_stream_event_line,
        preview_line,
    };

    #[test]
    fn parse_stream_event_line_accepts_valid_event() {
        let ev = parse_stream_event_line(r#"{"event":"text","data":{"text":"hello"}}"#)
            .expect("valid JSON-Lines event should parse");
        assert_eq!(ev.event, "text");
        assert_eq!(ev.data["text"], "hello");
    }

    #[test]
    fn parse_stream_event_line_rejects_non_json_noise() {
        let err = parse_stream_event_line("INFO booting agent")
            .expect_err("non-JSON noise should be rejected");
        assert!(err.contains("JSON parse error"));
    }

    #[test]
    fn parse_stream_event_line_rejects_missing_event_field() {
        let err = parse_stream_event_line(r#"{"data":{"text":"hello"}}"#)
            .expect_err("missing event should be rejected");
        assert!(err.contains("missing string field 'event'"));
    }

    #[test]
    fn preview_line_truncates_and_marks_suffix() {
        let preview = preview_line("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(preview, "abcdefghij...");
    }

    #[test]
    fn protocol_warning_event_contains_diagnostic_details() {
        let ev = make_protocol_warning_event(1, 3, "INFO booting agent", "JSON parse error");
        assert_eq!(ev.event, "protocol_warning");
        assert_eq!(ev.data["consecutive_invalid_lines"], 1);
        assert_eq!(ev.data["total_invalid_lines"], 3);
        assert_eq!(ev.data["line_preview"], "INFO booting agent");
    }

    #[test]
    fn protocol_recovered_event_reports_recovery_summary() {
        let ev = make_protocol_recovered_event(2, 5);
        assert_eq!(ev.event, "protocol_recovered");
        assert_eq!(ev.data["recovered_lines"], 2);
        assert_eq!(ev.data["total_invalid_lines"], 5);
    }
}

/// Kill the current chat subprocess if running. Called when user clicks "Stop".
/// Also clears pending confirmation/clarification channels so blocked recv() calls unblock.
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

// ─── Load recent data from ~/.skilllite/chat/ (no subprocess) ─────────────────

/// Task step from plan JSON.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanStep {
    pub id: u32,
    pub description: String,
    pub completed: bool,
}

/// Plan from ~/.skilllite/chat/plans/.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecentPlan {
    pub task: String,
    pub steps: Vec<PlanStep>,
}

/// Recent data: memory files, output files, latest plan.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecentData {
    pub memory_files: Vec<String>,
    pub output_files: Vec<String>,
    pub log_files: Vec<String>,
    pub plan: Option<RecentPlan>,
}

fn skilllite_chat_root() -> PathBuf {
    skilllite_core::paths::chat_root()
}

/// Open a directory in the system file manager.
/// module: "output" | "memory" | "plan" | "log" (log opens transcripts dir) | "evolution" (数据目录 ~/.skilllite)
pub fn open_directory(module: &str) -> Result<(), String> {
    let chat_root = skilllite_chat_root();
    let path = match module {
        "output" => chat_root.join("output"),
        "memory" => chat_root.join("memory"),
        "plan" => chat_root.join("plans"),
        "log" => chat_root.join("transcripts"), // 执行日志对应 transcript .jsonl 文件
        "evolution" => chat_root,
        _ => return Err(format!("Unknown module: {}", module)),
    };
    // Ensure directory exists so the file manager opens a valid path
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 在系统文件管理器中打开目录，或对文件执行「在文件夹中显示」。
pub fn reveal_in_file_manager(path_str: &str) -> Result<(), String> {
    let raw = std::path::PathBuf::from(path_str.trim());
    if !raw.is_absolute() {
        return Err("需要绝对路径".to_string());
    }
    let path = raw.clone();
    if !path.exists() {
        let rt = skilllite_sandbox::get_runtime_dir(None)
            .ok_or_else(|| "无法解析 SkillLite 运行时目录".to_string())?;
        if raw == rt {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            return Err("路径不存在".to_string());
        }
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    reveal_path_in_os(&path)
}

fn reveal_path_in_os(path: &std::path::Path) -> Result<(), String> {
    if path.is_dir() {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if path.is_file() {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            use std::ffi::OsString;
            let mut arg = OsString::from("/select,");
            arg.push(path.as_os_str());
            std::process::Command::new("explorer")
                .arg(arg)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let parent = path
                .parent()
                .ok_or_else(|| "无法解析文件所在目录".to_string())?;
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("不是有效的文件或目录".to_string())
}

/// (path, modified_time) pair for sorting by modification time.
type FileWithMtime = (String, std::time::SystemTime);

fn collect_md_files(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<FileWithMtime>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_md_files(&p, base, out);
        } else if p.extension().map_or(false, |e| e == "md") {
            if let Ok(rel) = p.strip_prefix(base) {
                let mtime = p
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                out.push((rel.to_string_lossy().to_string(), mtime));
            }
        }
    }
}

fn collect_output_files_inner(
    dir: &std::path::Path,
    base: &std::path::Path,
    out: &mut Vec<FileWithMtime>,
) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    const EXTS: &[&str] = &[
        "md", "html", "htm", "txt", "json", "csv", "png", "jpg", "jpeg", "gif", "webp", "svg",
    ];
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_output_files_inner(&p, base, out);
        } else if let Some(ext) = p.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if EXTS.contains(&ext_lower.as_str()) {
                if let Ok(rel) = p.strip_prefix(base) {
                    let mtime = p
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    out.push((rel.to_string_lossy().to_string(), mtime));
                }
            }
        }
    }
}

/// Sort by modification time descending (newest first) and return paths only.
fn sort_newest_first(mut items: Vec<FileWithMtime>) -> Vec<String> {
    items.sort_by(|a, b| b.1.cmp(&a.1));
    items.into_iter().map(|(path, _)| path).collect()
}

/// Load memory files from chat_root (for parallel execution).
fn load_memory_files(chat_root: &std::path::Path) -> Vec<String> {
    let memory_dir = chat_root.join("memory");
    let mut out = Vec::new();
    if memory_dir.exists() {
        collect_md_files(&memory_dir, &memory_dir, &mut out);
    }
    sort_newest_first(out)
}

/// Load transcript log files from chat_root/transcripts/, filtered to the last 3 days.
fn load_log_files(chat_root: &std::path::Path) -> Vec<String> {
    let transcripts_dir = chat_root.join("transcripts");
    if !transcripts_dir.is_dir() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(&transcripts_dir) else {
        return vec![];
    };
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(3 * 24 * 60 * 60);
    let mut out: Vec<FileWithMtime> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().map_or(true, |e| e != "jsonl") {
            continue;
        }
        let mtime = p
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if mtime < cutoff {
            continue;
        }
        if let Some(name) = p.file_name() {
            out.push((name.to_string_lossy().to_string(), mtime));
        }
    }
    sort_newest_first(out)
}

/// Read a single transcript/log file by filename (e.g. "default-2026-03-23.jsonl").
pub fn read_log_file(filename: &str) -> Result<String, String> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid filename".to_string());
    }
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("transcripts").join(filename);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

/// Load output files from chat_root (for parallel execution).
fn load_output_files(chat_root: &std::path::Path) -> Vec<String> {
    let output_dir = chat_root.join("output");
    let mut out = Vec::new();
    if output_dir.exists() {
        collect_output_files_inner(&output_dir, &output_dir, &mut out);
    }
    sort_newest_first(out)
}

/// Load latest plan from chat_root (for parallel execution).
/// Supports jsonl (append) format and legacy .json.
fn load_plan_data(chat_root: &std::path::Path) -> Option<RecentPlan> {
    let plans_dir = chat_root.join("plans");
    if !plans_dir.exists() {
        return None;
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let session_key = "default";

    fn parse_plan_from_file(path: &std::path::Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        match path.extension().and_then(|e| e.to_str()) {
            Some("jsonl") => content
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .and_then(|l| serde_json::from_str(l).ok()),
            _ => serde_json::from_str(&content).ok(),
        }
    }

    let plan: Option<serde_json::Value> = {
        let jsonl_path = plans_dir.join(format!("{}-{}.jsonl", session_key, today));
        let json_path = plans_dir.join(format!("{}-{}.json", session_key, today));
        if jsonl_path.exists() {
            parse_plan_from_file(&jsonl_path)
        } else if json_path.exists() {
            parse_plan_from_file(&json_path)
        } else {
            let mut candidates: Vec<_> = std::fs::read_dir(&plans_dir)
                .ok()?
                .flatten()
                .filter(|e| {
                    e.path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map_or(false, |n| n.starts_with(session_key))
                })
                .collect();
            candidates.sort_by_key(|e| {
                std::fs::metadata(e.path())
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });
            candidates
                .last()
                .and_then(|e| parse_plan_from_file(&e.path()))
        }
    };

    let plan = plan?;
    let task = plan
        .get("task")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let steps_arr = plan.get("steps").and_then(|s| s.as_array())?;
    let steps: Vec<PlanStep> = steps_arr
        .iter()
        .map(|s| {
            let id = s.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let desc = s.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let status = s
                .get("status")
                .and_then(|st| st.as_str())
                .unwrap_or("pending");
            PlanStep {
                id: if id > 0 { id } else { 1 },
                description: desc.to_string(),
                completed: status == "completed" || status == "done",
            }
        })
        .collect();
    Some(RecentPlan { task, steps })
}

/// Load recent memory files, output files, and plan in parallel using threads.
pub fn load_recent() -> RecentData {
    let chat_root = skilllite_chat_root();
    if !chat_root.exists() {
        return RecentData {
            memory_files: vec![],
            output_files: vec![],
            log_files: vec![],
            plan: None,
        };
    }

    let root = chat_root.clone();
    let mem_handle = std::thread::spawn(move || load_memory_files(&root));

    let root = chat_root.clone();
    let out_handle = std::thread::spawn(move || load_output_files(&root));

    let root = chat_root.clone();
    let log_handle = std::thread::spawn(move || load_log_files(&root));

    let plan_handle = std::thread::spawn(move || load_plan_data(&chat_root));

    let memory_files = mem_handle.join().unwrap_or_default();
    let output_files = out_handle.join().unwrap_or_default();
    let log_files = log_handle.join().unwrap_or_default();
    let plan = plan_handle.join().unwrap_or(None);

    RecentData {
        memory_files,
        output_files,
        log_files,
        plan,
    }
}

/// Read content of an output file by relative path (e.g. "report.md" or "index.html").
/// For text files returns the content; use read_output_file_base64 for binary (e.g. images).
pub fn read_output_file(relative_path: &str) -> Result<String, String> {
    if relative_path.contains("..") || relative_path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("output").join(relative_path);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

/// Read output file as base64 (for binary files like PNG, JPG, etc.).
pub fn read_output_file_base64(relative_path: &str) -> Result<String, String> {
    if relative_path.contains("..") || relative_path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("output").join(relative_path);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    let bytes = std::fs::read(&full_path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Read content of a memory file by relative path (e.g. "platforms/skilllite_analysis.md").
/// Path must be under memory/ and must not contain "..".
pub fn read_memory_file(relative_path: &str) -> Result<String, String> {
    if relative_path.contains("..") || relative_path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("memory").join(relative_path);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

// ─── Load transcript (chat history) for UI display ───────────────────────────

/// Single message entry for frontend display.
/// `role` determines the type: "user", "assistant", "tool_call", "tool_result".
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// List transcript file paths for session, sorted by date (legacy first, then YYYY-MM-DD).
fn list_transcript_paths(transcripts_dir: &std::path::Path, session_key: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let legacy = transcripts_dir.join(format!("{}.jsonl", session_key));
    if legacy.exists() {
        paths.push(legacy);
    }
    if !transcripts_dir.exists() {
        return paths;
    }
    let Ok(entries) = std::fs::read_dir(transcripts_dir) else {
        return paths;
    };
    let prefix = format!("{}-", session_key);
    let suffix = ".jsonl";
    for e in entries.flatten() {
        let path = e.path();
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(suffix) {
                paths.push(path);
            }
        }
    }
    paths.sort_by(|a, b| {
        let stem_a = a
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let stem_b = b
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let date_a = stem_a.strip_prefix(&prefix).unwrap_or("0000-00-00");
        let date_b = stem_b.strip_prefix(&prefix).unwrap_or("0000-00-00");
        date_a.cmp(date_b)
    });
    paths
}

/// Raw entry from transcript JSONL (minimal parse for message/compaction/tool_call/tool_result).
#[derive(Clone)]
struct TranscriptEntryRaw {
    ty: String,
    id: String,
    role: String,
    content: String,
    summary: Option<String>,
    name: Option<String>,
    is_error: Option<bool>,
}

/// Load chat transcript messages for display. Returns user/assistant messages in order.
/// When compaction exists: shows summary + only messages after compaction (matches agent context).
pub fn load_transcript(session_key: &str) -> Vec<TranscriptMessage> {
    let chat_root = skilllite_chat_root();
    if !chat_root.exists() {
        return vec![];
    }
    let transcripts_dir = chat_root.join("transcripts");
    let paths = list_transcript_paths(&transcripts_dir, session_key);
    let mut entries: Vec<TranscriptEntryRaw> = Vec::new();

    for path in paths {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = match serde_json::from_str(line) {
                Ok(x) => x,
                Err(_) => continue,
            };
            let ty = v
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if ty == "message" {
                let role = v
                    .get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                if role != "user" && role != "assistant" {
                    continue;
                }
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string(),
                    role,
                    content: v
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    summary: None,
                    name: None,
                    is_error: None,
                });
            } else if ty == "tool_call" {
                let name = v
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = v
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string();
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string(),
                    role: "tool_call".to_string(),
                    content: arguments,
                    summary: None,
                    name: Some(name),
                    is_error: None,
                });
            } else if ty == "tool_result" {
                let name = v
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let result = v
                    .get("result")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string();
                let is_error = v
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string(),
                    role: "tool_result".to_string(),
                    content: result,
                    summary: None,
                    name: Some(name),
                    is_error: Some(is_error),
                });
            } else if ty == "compaction" {
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: String::new(),
                    role: String::new(),
                    content: String::new(),
                    summary: v.get("summary").and_then(|s| s.as_str()).map(String::from),
                    name: None,
                    is_error: None,
                });
            }
        }
    }

    let compaction_idx = entries.iter().rposition(|e| e.ty == "compaction");
    let (to_use, summary_opt) = match compaction_idx {
        Some(idx) => (entries[idx + 1..].to_vec(), entries[idx].summary.clone()),
        None => (entries, None),
    };

    let mut messages = Vec::new();
    if let Some(summary) = summary_opt {
        if !summary.is_empty() {
            messages.push(TranscriptMessage {
                id: "compaction".to_string(),
                role: "assistant".to_string(),
                content: format!("[此前对话已压缩]\n\n{}", summary),
                name: None,
                is_error: None,
            });
        }
    }
    for (i, e) in to_use.iter().enumerate() {
        let dominated_by_type =
            e.ty == "message" || e.ty == "tool_call" || e.ty == "tool_result";
        if !dominated_by_type {
            continue;
        }
        if e.ty == "message" && e.content.is_empty() && e.role != "user" {
            continue;
        }
        let id = if e.id.is_empty() {
            format!("msg-{}", i)
        } else {
            e.id.clone()
        };
        messages.push(TranscriptMessage {
            id,
            role: e.role.clone(),
            content: e.content.clone(),
            name: e.name.clone(),
            is_error: e.is_error,
        });
    }
    messages
}

/// Clear session (OpenClaw-style): summarize to memory, archive transcript, reset counts.
/// Spawns `skilllite clear-session` so short conversations are preserved in memory/ before clearing.
pub fn clear_transcript(
    session_key: &str,
    workspace: &str,
    skilllite_path: &std::path::Path,
) -> Result<(), String> {
    let workspace_root = find_project_root(workspace);

    let mut cmd = std::process::Command::new(&skilllite_path);
    cmd.args([
        "clear-session",
        "--session-key",
        session_key,
        "--workspace",
        workspace_root.to_string_lossy().as_ref(),
    ])
    .current_dir(&workspace_root)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run clear-session: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "clear-session failed: {}",
            if stderr.is_empty() {
                output.status.to_string()
            } else {
                stderr.trim().to_string()
            }
        ));
    }
    Ok(())
}

pub(crate) fn resolve_skilllite_path_app(app: &tauri::AppHandle) -> std::path::PathBuf {
    let exe_name = if cfg!(target_os = "windows") {
        "skilllite.exe"
    } else {
        "skilllite"
    };

    // 1. Tauri resource_dir (works in production bundles)
    if let Ok(res_dir) = app.path().resource_dir() {
        let bundled = res_dir.join(exe_name);
        if bundled.exists() {
            eprintln!(
                "[skilllite-bridge] using bundled binary: {}",
                bundled.display()
            );
            return bundled;
        }
    }

    // 2. ~/.skilllite/bin (where prebuild installs for dev mode)
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            eprintln!(
                "[skilllite-bridge] using ~/.skilllite/bin binary: {}",
                dev_bin.display()
            );
            return dev_bin;
        }
    }

    // 3. Fallback: rely on PATH
    eprintln!(
        "[skilllite-bridge] falling back to PATH lookup for '{}'",
        exe_name
    );
    std::path::PathBuf::from(exe_name)
}

// ─── List skills & repair-skills (evolution) ───────────────────────────────────

/// Whether the skill dir has any script file (scripts/ or root with common script extensions).
fn skill_has_scripts(path: &std::path::Path) -> bool {
    let scripts_dir = path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            if entries
                .filter(|e| e.as_ref().ok().map(|e| e.path().is_file()).unwrap_or(false))
                .count()
                > 0
            {
                return true;
            }
        }
    }
    const EXTS: &[&str] = &["py", "js", "ts", "sh", "bash"];
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if EXTS.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Collect skill (dir_path, name) from a root dir, same shape as evolution validate (including _evolved/_pending).
fn collect_skill_dirs(root: &std::path::Path) -> Vec<(PathBuf, String)> {
    if !root.exists() || !root.is_dir() {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_evolved" || name == "_pending" {
                for e2 in std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                {
                    let p2 = e2.path();
                    let sub = e2.file_name().to_string_lossy().into_owned();
                    if !p2.is_dir() {
                        continue;
                    }
                    if p2.join("SKILL.md").exists() && skill_has_scripts(&p2) {
                        dirs.push((p2, sub));
                    } else if sub == "_pending" {
                        for e3 in std::fs::read_dir(&p2)
                            .ok()
                            .into_iter()
                            .flatten()
                            .filter_map(|e| e.ok())
                        {
                            let p3 = e3.path();
                            if p3.is_dir() && p3.join("SKILL.md").exists() && skill_has_scripts(&p3)
                            {
                                dirs.push((p3, e3.file_name().to_string_lossy().into_owned()));
                            }
                        }
                    }
                }
            } else if path.join("SKILL.md").exists() && skill_has_scripts(&path) {
                dirs.push((path, name));
            }
            continue;
        }
        if path.join("SKILL.md").exists() && skill_has_scripts(&path) {
            dirs.push((path, name));
        }
    }
    dirs
}

/// List skill names in workspace (for repair UI). Uses same logic as evolution: .skills and skills, incl. _evolved/_pending.
pub fn list_skill_names(workspace: &str) -> Vec<String> {
    let root = find_project_root(workspace);
    let mut names = std::collections::HashSet::new();
    for skills_sub in [".skills", "skills"] {
        let dir = root.join(skills_sub);
        for (_, name) in collect_skill_dirs(&dir) {
            names.insert(name);
        }
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Resolve skill directory path by name (searches .skills and skills, incl. _evolved/_pending). Returns None if not found.
fn find_skill_dir(workspace: &str, skill_name: &str) -> Option<std::path::PathBuf> {
    let root = find_project_root(workspace);
    for skills_sub in [".skills", "skills"] {
        let dir = root.join(skills_sub);
        for (path, name) in collect_skill_dirs(&dir) {
            if name == skill_name {
                return Some(path);
            }
        }
    }
    None
}

/// Open the given skill's directory in the system file manager.
pub fn open_skill_directory(workspace: &str, skill_name: &str) -> Result<(), String> {
    let path = find_skill_dir(workspace, skill_name)
        .ok_or_else(|| format!("未找到技能目录: {}", skill_name))?;
    if !path.exists() || !path.is_dir() {
        return Err(format!("技能目录不存在: {}", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Run `skilllite evolution repair-skills [skill_names...]`. If skill_names is empty, repairs all failed; otherwise only those.
pub fn repair_skills(
    workspace: &str,
    skill_names: &[String],
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);

    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("evolution").arg("repair-skills");
    for name in skill_names {
        cmd.arg(name);
    }
    cmd.arg("--from-source");
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 repair-skills 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(combined)
}

/// Run `skilllite add <source>` in the workspace. Installs to workspace .skills (creates if needed).
/// Source: owner/repo, owner/repo@skill-name, https://github.com/..., or local path.
pub fn add_skill(
    workspace: &str,
    source: &str,
    force: bool,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);
    let source = source.trim();
    if source.is_empty() {
        return Err("请填写来源，例如：owner/repo 或 owner/repo@skill-name".to_string());
    }

    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("add")
        .arg(source)
        .arg("--skills-dir")
        .arg(".skills");
    if force {
        cmd.arg("--force");
    }
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite add 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(summarise_add_output(&combined))
}

/// 从 skilllite add 的完整输出中提取简短摘要，避免在桌面端刷屏。
fn summarise_add_output(output: &str) -> String {
    if output.is_empty() {
        return "已添加".to_string();
    }
    // 匹配 "🎉 Successfully added 14 skill(s) from obra/superpowers" 或 "Successfully added 1 skill(s)"
    let line = output
        .lines()
        .find(|line| line.contains("Successfully added") && line.contains("skill(s)"));
    if let Some(line) = line {
        let line = line.trim().trim_start_matches("🎉 ").trim();
        if let Some(after) = line.strip_prefix("Successfully added ") {
            let num_str = after.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<u32>() {
                let from = after.split(" from ").nth(1).map(str::trim);
                return if let Some(src) = from {
                    format!("已添加 {} 个技能（来自 {}）", n, src)
                } else {
                    format!("已添加 {} 个技能", n)
                };
            }
        }
    }
    "已添加".to_string()
}

// ─── Evolution status & pending skill review (desktop) ───────────────────────

fn workspace_env_lookup(workspace: &str, key: &str) -> Option<String> {
    load_dotenv_for_child(workspace)
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .or_else(|| std::env::var(key).ok())
}

fn evolution_mode_from_workspace(workspace: &str) -> skilllite_evolution::EvolutionMode {
    use skilllite_core::config::env_keys::evolution as evo_env;
    match workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION).as_deref() {
        None | Some("1") | Some("true") | Some("") => skilllite_evolution::EvolutionMode::All,
        Some("0") | Some("false") => skilllite_evolution::EvolutionMode::Disabled,
        Some("prompts") => skilllite_evolution::EvolutionMode::PromptsOnly,
        Some("memory") => skilllite_evolution::EvolutionMode::MemoryOnly,
        Some("skills") => skilllite_evolution::EvolutionMode::SkillsOnly,
        _ => skilllite_evolution::EvolutionMode::All,
    }
}

fn evolution_mode_labels(mode: &skilllite_evolution::EvolutionMode) -> (&'static str, &'static str) {
    match mode {
        skilllite_evolution::EvolutionMode::All => ("all", "全部启用"),
        skilllite_evolution::EvolutionMode::PromptsOnly => ("prompts", "仅 Prompts"),
        skilllite_evolution::EvolutionMode::MemoryOnly => ("memory", "仅 Memory"),
        skilllite_evolution::EvolutionMode::SkillsOnly => ("skills", "仅 Skills"),
        skilllite_evolution::EvolutionMode::Disabled => ("disabled", "已禁用"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionLogEntryDto {
    pub ts: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub target_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatusPayload {
    pub mode_key: String,
    pub mode_label: String,
    pub interval_secs: u64,
    pub decision_threshold: i64,
    pub unprocessed_decisions: i64,
    pub last_run_ts: Option<String>,
    pub judgement_label: Option<String>,
    pub judgement_reason: Option<String>,
    pub recent_events: Vec<EvolutionLogEntryDto>,
    pub pending_skill_count: usize,
    pub db_error: Option<String>,
}

/// Evolution feedback DB + schedule hints for the assistant UI.
pub fn load_evolution_status(workspace: &str) -> EvolutionStatusPayload {
    use skilllite_core::config::env_keys::evolution as evo_env;
    let mode = evolution_mode_from_workspace(workspace);
    let (mode_key, mode_label) = evolution_mode_labels(&mode);

    let interval_secs: u64 = workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let decision_threshold: i64 =
        workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

    let chat_root = skilllite_core::paths::chat_root();
    let mut pending_skill_count = 0;
    let skills_root = find_project_root(workspace).join(".skills");
    if skills_root.is_dir() {
        pending_skill_count =
            skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root).len();
    }

    let mut db_error = None;
    let mut unprocessed_decisions = 0i64;
    let mut recent_events = Vec::new();
    let mut last_run_ts = None;
    let mut judgement_label = None;
    let mut judgement_reason = None;

    match skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        Ok(conn) => {
            if let Ok(c) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) {
                unprocessed_decisions = c;
            }
            if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn) {
                judgement_label = Some(summary.judgement.label_zh().to_string());
                judgement_reason = Some(summary.reason);
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts FROM evolution_log WHERE type = 'evolution_run' ORDER BY ts DESC LIMIT 1",
            ) {
                if let Ok(mut rows) = stmt.query([]) {
                    if let Ok(Some(row)) = rows.next() {
                        last_run_ts = row.get(0).ok();
                    }
                }
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts, type, target_id, reason FROM evolution_log ORDER BY ts DESC LIMIT 25",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok(EvolutionLogEntryDto {
                        ts: row.get(0)?,
                        event_type: row.get(1)?,
                        target_id: row.get::<_, Option<String>>(2)?,
                        reason: row.get::<_, Option<String>>(3)?,
                    })
                }) {
                    recent_events.extend(rows.flatten());
                }
            }
        }
        Err(e) => {
            db_error = Some(format!("无法打开进化数据库: {}", e));
        }
    }

    EvolutionStatusPayload {
        mode_key: mode_key.to_string(),
        mode_label: mode_label.to_string(),
        interval_secs,
        decision_threshold,
        unprocessed_decisions,
        last_run_ts,
        judgement_label,
        judgement_reason,
        recent_events,
        pending_skill_count,
        db_error,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingSkillDto {
    pub name: String,
    pub needs_review: bool,
    pub preview: String,
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

pub fn list_evolution_pending_skills(workspace: &str) -> Vec<PendingSkillDto> {
    let skills_root = find_project_root(workspace).join(".skills");
    if !skills_root.is_dir() {
        return Vec::new();
    }
    skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root)
        .into_iter()
        .map(|(name, needs_review)| {
            let path = skills_root
                .join("_evolved")
                .join("_pending")
                .join(&name)
                .join("SKILL.md");
            let preview = std::fs::read_to_string(&path)
                .map(|s| truncate_utf8(&s, 4000))
                .unwrap_or_default();
            PendingSkillDto {
                name,
                needs_review,
                preview,
            }
        })
        .collect()
}

pub fn read_evolution_pending_skill_md(workspace: &str, skill_name: &str) -> Result<String, String> {
    let skills_root = find_project_root(workspace).join(".skills");
    let path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name)
        .join("SKILL.md");
    if !path.is_file() {
        return Err(format!("未找到待审核技能: {}", skill_name));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn evolution_confirm_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = find_project_root(workspace).join(".skills");
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())?;
    let chat_root = skilllite_core::paths::chat_root();
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &chat_root,
            "skill_confirmed",
            skill_name,
            "user confirmed (assistant)",
            "",
        );
    }
    Ok(())
}

pub fn evolution_reject_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = find_project_root(workspace).join(".skills");
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())
}

// ─── Onboarding: init workspace, probe Ollama ─────────────────────────────────

/// Requested provider during onboarding health check.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingProvider {
    Api,
    Ollama,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheckItem {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OnboardingHealthCheckResult {
    pub binary: HealthCheckItem,
    pub provider: HealthCheckItem,
    pub workspace: HealthCheckItem,
    pub data_dir: HealthCheckItem,
    pub ok: bool,
}

pub fn run_onboarding_health_check(
    skilllite_path: &std::path::Path,
    workspace: &str,
    provider: OnboardingProvider,
    api_key: Option<&str>,
) -> OnboardingHealthCheckResult {
    let binary = check_bundled_skilllite(skilllite_path);
    let provider = check_provider(provider, api_key);
    let workspace = check_workspace(workspace);
    let data_dir = check_data_dir();
    let ok = binary.ok && provider.ok && workspace.ok && data_dir.ok;
    OnboardingHealthCheckResult {
        binary,
        provider,
        workspace,
        data_dir,
        ok,
    }
}

fn check_bundled_skilllite(skilllite_path: &std::path::Path) -> HealthCheckItem {
    if !skilllite_path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("未找到 SkillLite 二进制：{}", skilllite_path.display()),
        };
    }

    match std::process::Command::new(skilllite_path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => HealthCheckItem {
            ok: true,
            message: format!("内置引擎可用：{}", skilllite_path.display()),
        },
        Ok(status) => HealthCheckItem {
            ok: false,
            message: format!("内置引擎启动失败（状态：{}）", status),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法启动内置引擎：{}", e),
        },
    }
}

fn check_provider(provider: OnboardingProvider, api_key: Option<&str>) -> HealthCheckItem {
    match provider {
        OnboardingProvider::Api => {
            let has_key = api_key.map(|k| !k.trim().is_empty()).unwrap_or(false);
            if has_key {
                HealthCheckItem {
                    ok: true,
                    message: "已填写 API Key，可使用云模型".to_string(),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "尚未填写 API Key".to_string(),
                }
            }
        }
        OnboardingProvider::Ollama => {
            let result = probe_ollama();
            if result.available && result.models.iter().any(|m| !m.contains("embed")) {
                HealthCheckItem {
                    ok: true,
                    message: format!("本机 Ollama 可用，检测到 {} 个模型", result.models.len()),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "未检测到可用的 Ollama 聊天模型".to_string(),
                }
            }
        }
    }
}

fn check_workspace(workspace: &str) -> HealthCheckItem {
    let path = std::path::Path::new(workspace);
    if !path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不存在：{}", path.display()),
        };
    }
    if !path.is_dir() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不是目录：{}", path.display()),
        };
    }

    let probe_dir = path.join(".skilllite");
    match std::fs::create_dir_all(&probe_dir) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("工作区可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("工作区不可写：{}", e),
        },
    }
}

fn check_data_dir() -> HealthCheckItem {
    let path = skilllite_core::paths::data_root();
    match std::fs::create_dir_all(&path) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("数据目录可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法创建数据目录：{}", e),
        },
    }
}

/// Run `skilllite init` in the given directory. Creates .skills and example content.
pub fn init_workspace(dir: &str, skilllite_path: &std::path::Path) -> Result<(), String> {
    let path = std::path::Path::new(dir);
    if !path.is_dir() {
        return Err("目录不存在".to_string());
    }
    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("init").current_dir(path);
    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite init 失败: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

pub use skilllite_sandbox::{RuntimeUiLine, RuntimeUiSnapshot};

/// Python/Node 来源探测（系统 PATH vs SkillLite 缓存下载），供左侧栏等 UI 展示。
pub fn probe_runtime_status() -> RuntimeUiSnapshot {
    skilllite_sandbox::probe_runtime_for_ui(None)
}

/// Result of probing local Ollama (localhost:11434).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OllamaProbeResult {
    pub available: bool,
    /// All installed model names.
    pub models: Vec<String>,
    /// Whether an embedding-capable model is present (name contains "embed").
    pub has_embedding: bool,
}

/// Probe Ollama at localhost:11434; returns availability, all model names, and embedding support.
pub fn probe_ollama() -> OllamaProbeResult {
    let empty = OllamaProbeResult {
        available: false,
        models: vec![],
        has_embedding: false,
    };
    let body = match ollama_get_tags() {
        Ok(b) => b,
        Err(_) => return empty,
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(j) => j,
        Err(_) => return empty,
    };
    let arr = match json.get("models").and_then(|m| m.as_array()) {
        Some(a) => a,
        None => {
            return OllamaProbeResult {
                available: true,
                models: vec![],
                has_embedding: false,
            }
        }
    };
    let models: Vec<String> = arr
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string())
        .collect();
    let has_embedding = models.iter().any(|n| n.contains("embed"));
    OllamaProbeResult {
        available: true,
        models,
        has_embedding,
    }
}

// ─── Session management ────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionInfo {
    pub session_key: String,
    pub display_name: String,
    pub updated_at: String,
    pub message_preview: Option<String>,
}

fn sessions_json_path() -> PathBuf {
    skilllite_chat_root().join("sessions.json")
}

fn get_last_user_message_from_transcripts(
    transcripts_dir: &std::path::Path,
    session_key: &str,
) -> Option<String> {
    let paths = list_transcript_paths(transcripts_dir, session_key);
    for path in paths.into_iter().rev() {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut last_msg = None;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v.get("type").and_then(|t| t.as_str()) == Some("message")
                && v.get("role").and_then(|r| r.as_str()) == Some("user")
            {
                if let Some(c) = v.get("content").and_then(|c| c.as_str()) {
                    let preview: String = c.chars().take(40).collect();
                    last_msg = Some(if c.chars().count() > 40 {
                        format!("{}…", preview)
                    } else {
                        preview
                    });
                }
            }
        }
        if last_msg.is_some() {
            return last_msg;
        }
    }
    None
}

pub fn list_sessions() -> Vec<SessionInfo> {
    let path = sessions_json_path();
    let store: serde_json::Value = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({}))
    } else {
        json!({})
    };

    let sessions_map = store
        .get("sessions")
        .and_then(|s| s.as_object())
        .cloned()
        .unwrap_or_default();

    let chat_root = skilllite_chat_root();
    let transcripts_dir = chat_root.join("transcripts");

    let mut result: Vec<SessionInfo> = sessions_map
        .iter()
        .map(|(key, val)| {
            let display_name = val
                .get("display_name")
                .and_then(|d| d.as_str())
                .unwrap_or(if key == "default" { "默认会话" } else { key })
                .to_string();
            let updated_at = val
                .get("updated_at")
                .and_then(|u| u.as_str())
                .unwrap_or("0")
                .to_string();
            let message_preview =
                get_last_user_message_from_transcripts(&transcripts_dir, key);
            SessionInfo {
                session_key: key.clone(),
                display_name,
                updated_at,
                message_preview,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        let a_ts: u64 = a.updated_at.parse().unwrap_or(0);
        let b_ts: u64 = b.updated_at.parse().unwrap_or(0);
        b_ts.cmp(&a_ts)
    });

    if !result.iter().any(|s| s.session_key == "default") {
        let preview = get_last_user_message_from_transcripts(&transcripts_dir, "default");
        result.push(SessionInfo {
            session_key: "default".to_string(),
            display_name: "默认会话".to_string(),
            updated_at: "0".to_string(),
            message_preview: preview,
        });
    }

    result
}

pub fn create_session(display_name: &str) -> Result<SessionInfo, String> {
    let path = sessions_json_path();
    let mut store: serde_json::Value = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({ "sessions": {} }))
    } else {
        json!({ "sessions": {} })
    };

    let sessions = store
        .get_mut("sessions")
        .and_then(|s| s.as_object_mut())
        .ok_or_else(|| "Invalid sessions.json format".to_string())?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let session_key = format!("s-{:x}", ts);
    let now = format!("{}", ts / 1000);

    sessions.insert(
        session_key.clone(),
        json!({
            "session_id": format!("tx-{:x}", ts),
            "session_key": session_key,
            "display_name": display_name,
            "updated_at": now,
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0,
            "context_tokens": 0,
            "compaction_count": 0
        }),
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(SessionInfo {
        session_key,
        display_name: display_name.to_string(),
        updated_at: now,
        message_preview: None,
    })
}

pub fn rename_session(session_key: &str, new_name: &str) -> Result<(), String> {
    let path = sessions_json_path();
    let mut store: serde_json::Value = if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({ "sessions": {} }))
    } else {
        json!({ "sessions": {} })
    };

    if store.get("sessions").and_then(|s| s.as_object()).is_none() {
        store["sessions"] = json!({});
    }

    let sessions = store
        .get_mut("sessions")
        .and_then(|s| s.as_object_mut())
        .ok_or_else(|| "Invalid sessions.json format".to_string())?;

    match sessions.get_mut(session_key) {
        Some(entry) => {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("display_name".to_string(), json!(new_name));
            }
        }
        None if session_key == "default" => {
            sessions.insert(
                session_key.to_string(),
                json!({
                    "display_name": new_name,
                    "updated_at": "0",
                }),
            );
        }
        None => {
            return Err(format!("Session '{}' not found", session_key));
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn delete_session(session_key: &str) -> Result<(), String> {
    if session_key == "default" {
        return Err("不能删除默认会话".to_string());
    }

    let path = sessions_json_path();
    if path.exists() {
        let mut store: serde_json::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({ "sessions": {} }));

        if let Some(sessions) = store.get_mut("sessions").and_then(|s| s.as_object_mut()) {
            sessions.remove(session_key);
        }

        let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    }

    let chat_root = skilllite_chat_root();
    let transcripts_dir = chat_root.join("transcripts");
    if transcripts_dir.is_dir() {
        for p in list_transcript_paths(&transcripts_dir, session_key) {
            let _ = std::fs::remove_file(p);
        }
    }

    let plans_dir = chat_root.join("plans");
    if plans_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&plans_dir) {
            let prefix = format!("{}-", session_key);
            let exact = format!("{}.json", session_key);
            let exact_jsonl = format!("{}.jsonl", session_key);
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) || name == exact || name == exact_jsonl {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }

    Ok(())
}

// ─── Memory summaries ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryEntry {
    pub path: String,
    pub title: String,
    pub summary: String,
    pub updated_at: String,
}

pub fn load_memory_summaries() -> Vec<MemoryEntry> {
    let chat_root = skilllite_chat_root();
    let memory_dir = chat_root.join("memory");
    if !memory_dir.exists() {
        return vec![];
    }

    let mut files: Vec<FileWithMtime> = Vec::new();
    collect_md_files(&memory_dir, &memory_dir, &mut files);
    files.sort_by(|a, b| b.1.cmp(&a.1));

    files
        .into_iter()
        .take(30)
        .map(|(rel_path, mtime)| {
            let full_path = memory_dir.join(&rel_path);
            let content = std::fs::read_to_string(&full_path).unwrap_or_default();
            let title = content
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim_start_matches('#').trim().to_string())
                .unwrap_or_else(|| rel_path.clone());
            let summary: String = content
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            let summary = if summary.chars().count() > 120 {
                format!("{}…", summary.chars().take(120).collect::<String>())
            } else {
                summary
            };

            let updated_secs = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            MemoryEntry {
                path: rel_path,
                title,
                summary,
                updated_at: format!("{}", updated_secs),
            }
        })
        .collect()
}

fn ollama_get_tags() -> Result<String, ()> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = ([127, 0, 0, 1], 11434).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| ())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| ())?;

    let req = b"GET /api/tags HTTP/1.1\r\nHost: localhost:11434\r\nConnection: close\r\n\r\n";
    stream.write_all(req).map_err(|_| ())?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|_| ())?;
    let s = String::from_utf8_lossy(&buf);
    let body = s.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    Ok(body.to_string())
}
