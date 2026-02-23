//! SkillLite bridge: spawn `skilllite agent-rpc` subprocess and forward JSON-RPC over stdio.
//!
//! Protocol: see crates/skilllite-agent/src/rpc.rs
//! Request: {"method":"agent_chat","params":{"message":"...","session_key":"default","config":{"workspace":"..."}}}
//! Response: JSON-Lines events (text_chunk, text, done, error, confirmation_request, etc.)
//!
//! For confirmation_request: emit to frontend, wait for skilllite_confirm, write to stdin.

use serde::Serialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager, Window};

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub event: String,
    pub data: serde_json::Value,
}

/// Shared state for confirmation flow: frontend calls skilllite_confirm → sends to this channel.
#[derive(Default, Clone)]
pub struct ConfirmationState(pub Arc<Mutex<Option<mpsc::Sender<bool>>>>);

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

/// Load .env from workspace and parents into env map for subprocess.
fn load_dotenv_for_child(workspace: &str) -> Vec<(String, String)> {
    use std::path::Path;
    let mut vars = Vec::new();
    let mut dir = Path::new(workspace)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(workspace).to_path_buf());
    for _ in 0..5 {
        let env_path = dir.join(".env");
        if let Ok(content) = std::fs::read_to_string(&env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(eq_pos) = line.find('=') {
                    let key = line[..eq_pos].trim().to_string();
                    let mut value = line[eq_pos + 1..].trim();
                    if value.starts_with('"') && value.ends_with('"') {
                        value = &value[1..value.len() - 1];
                    } else if value.starts_with('\'') && value.ends_with('\'') {
                        value = &value[1..value.len() - 1];
                    }
                    if !key.is_empty() {
                        vars.push((key, value.to_string()));
                    }
                }
            }
            break;
        }
        if !dir.pop() {
            break;
        }
    }
    vars
}

/// Config overrides from frontend (optional).
#[derive(serde::Deserialize, Default)]
pub struct ChatConfigOverrides {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub api_base: Option<String>,
    pub workspace: Option<String>,
}

/// Resolve skilllite binary path: bundled resource first, else "skilllite" from PATH.
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
    (PathBuf::from("skilllite"), false)
}

/// Run agent_chat via skilllite agent-rpc subprocess, emitting events to the window.
/// Loads .env from workspace (or parent dirs) so OPENAI_API_KEY etc. are available.
/// Config overrides (api_key, model, etc.) are merged into the request.
/// For confirmation_request: emits to frontend, waits for skilllite_confirm, writes to stdin.
/// Uses bundled skilllite if available (single-file distribution), else PATH.
pub fn chat_stream(
    window: Window,
    message: String,
    workspace: Option<String>,
    config_overrides: Option<ChatConfigOverrides>,
    confirmation_state: ConfirmationState,
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
    if let Some(ref cfg) = config_overrides {
        if let Some(ref key) = cfg.api_key {
            if !key.is_empty() {
                cmd.env("OPENAI_API_KEY", key);
            }
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
    }

    let request = json!({
        "method": "agent_chat",
        "params": {
            "message": message,
            "session_key": "default",
            "config": config_json
        }
    });
    writeln!(stdin, "{}", request).map_err(|e| e.to_string())?;
    // Keep stdin open for confirmation responses

    // Read stdout in a thread and emit events
    let (tx, rx) = mpsc::channel::<Result<StreamEvent, String>>();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
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
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(v) => {
                    let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
                    let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
                    if let Err(_) = tx.send(Ok(StreamEvent {
                        event: event.to_string(),
                        data,
                    })) {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(format!("JSON parse error: {}", e)));
                    break;
                }
            }
        }
    });

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
                        json!({ "prompt": prompt }),
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
                if let Err(e) = window.emit("skilllite-event", &ev) {
                    eprintln!("emit error: {}", e);
                }
                if ev.event == "done" || ev.event == "error" {
                    break;
                }
            }
            Err(e) => {
                let _ = window.emit(
                    "skilllite-event",
                    StreamEvent {
                        event: "error".to_string(),
                        data: json!({ "message": e }),
                    },
                );
                break;
            }
        }
    }

    drop(stdin);
    let _ = child.wait();
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
    pub plan: Option<RecentPlan>,
}

fn skilllite_chat_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".skilllite").join("chat"))
}

fn collect_md_files(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
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
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

/// Load recent memory files and latest plan from ~/.skilllite/chat/.
pub fn load_recent() -> RecentData {
    let chat_root = match skilllite_chat_root() {
        Some(r) if r.exists() => r,
        _ => {
            return RecentData {
                memory_files: vec![],
                output_files: vec![],
                plan: None,
            }
        }
    };

    let memory_dir = chat_root.join("memory");
    let mut memory_files = Vec::new();
    if memory_dir.exists() {
        collect_md_files(&memory_dir, &memory_dir, &mut memory_files);
    }
    memory_files.sort();

    let output_dir = chat_root.join("output");
    let mut output_files = Vec::new();
    if output_dir.exists() {
        fn collect_output_files(
            dir: &std::path::Path,
            base: &std::path::Path,
            out: &mut Vec<String>,
        ) {
            if !dir.is_dir() {
                return;
            }
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            const EXTS: &[&str] = &["md", "html", "htm", "txt", "json", "csv"];
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect_output_files(&p, base, out);
                } else if let Some(ext) = p.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if EXTS.contains(&ext_lower.as_str()) {
                        if let Ok(rel) = p.strip_prefix(base) {
                            out.push(rel.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        collect_output_files(&output_dir, &output_dir, &mut output_files);
        output_files.sort();
    }

    let plans_dir = chat_root.join("plans");
    let plan = if plans_dir.exists() {
        let session_key = "default";
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let path = plans_dir.join(format!("{}-{}.json", session_key, today));
        if !path.exists() {
            let mut candidates: Vec<_> = std::fs::read_dir(&plans_dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
                .collect();
            candidates.sort_by_key(|e| {
                std::fs::metadata(e.path())
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });
            candidates.last().map(|e| e.path())
        } else {
            Some(path)
        }
    } else {
        None
    };

    let plan_data = plan.and_then(|p| {
        let content = std::fs::read_to_string(&p).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        let task = v
            .get("task")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let steps_arr = v.get("steps").and_then(|s| s.as_array())?;
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
    });

    RecentData {
        memory_files,
        output_files,
        plan: plan_data,
    }
}

/// Read content of an output file by relative path (e.g. "report.md" or "index.html").
pub fn read_output_file(relative_path: &str) -> Result<String, String> {
    if relative_path.contains("..") || relative_path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    let chat_root = skilllite_chat_root().ok_or("Chat root not found")?;
    let full_path = chat_root.join("output").join(relative_path);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

/// Read content of a memory file by relative path (e.g. "platforms/skilllite_analysis.md").
/// Path must be under memory/ and must not contain "..".
pub fn read_memory_file(relative_path: &str) -> Result<String, String> {
    if relative_path.contains("..") || relative_path.starts_with('/') {
        return Err("Invalid path".to_string());
    }
    let chat_root = skilllite_chat_root().ok_or("Chat root not found")?;
    let full_path = chat_root.join("memory").join(relative_path);
    if !full_path.starts_with(chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

// ─── Load transcript (chat history) for UI display ───────────────────────────

/// Single message entry for frontend display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptMessage {
    pub id: String,
    pub role: String,   // "user" | "assistant"
    pub content: String,
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
        let stem_a = a.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
        let stem_b = b.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
        let date_a = stem_a.strip_prefix(&prefix).unwrap_or("0000-00-00");
        let date_b = stem_b.strip_prefix(&prefix).unwrap_or("0000-00-00");
        date_a.cmp(date_b)
    });
    paths
}

/// Raw entry from transcript JSONL (minimal parse for message/compaction).
#[derive(Clone)]
struct TranscriptEntryRaw {
    ty: String,
    id: String,
    role: String,
    content: String,
    summary: Option<String>,
}

/// Load chat transcript messages for display. Returns user/assistant messages in order.
/// When compaction exists: shows summary + only messages after compaction (matches agent context).
pub fn load_transcript(session_key: &str) -> Vec<TranscriptMessage> {
    let chat_root = match skilllite_chat_root() {
        Some(r) if r.exists() => r,
        _ => return vec![],
    };
    let transcripts_dir = chat_root.join("transcripts");
    let paths = list_transcript_paths(&transcripts_dir, session_key);
    let mut entries: Vec<TranscriptEntryRaw> = Vec::new();

    for path in paths {
        let Ok(file) = std::fs::File::open(&path) else { continue };
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
            let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if ty == "message" {
                let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("").to_string();
                if role != "user" && role != "assistant" {
                    continue;
                }
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    role,
                    content: v
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    summary: None,
                });
            } else if ty == "compaction" {
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: String::new(),
                    role: String::new(),
                    content: String::new(),
                    summary: v.get("summary").and_then(|s| s.as_str()).map(String::from),
                });
            }
        }
    }

    let compaction_idx = entries.iter().rposition(|e| e.ty == "compaction");
    let (to_use, summary_opt) = match compaction_idx {
        Some(idx) => (
            entries[idx + 1..].to_vec(),
            entries[idx].summary.clone(),
        ),
        None => (entries, None),
    };

    let mut messages = Vec::new();
    if let Some(summary) = summary_opt {
        if !summary.is_empty() {
            messages.push(TranscriptMessage {
                id: "compaction".to_string(),
                role: "assistant".to_string(),
                content: format!("[此前对话已压缩]\n\n{}", summary),
            });
        }
    }
    for (i, e) in to_use.iter().enumerate() {
        if e.ty == "message" && (!e.content.is_empty() || e.role == "user") {
            let id = if e.id.is_empty() {
                format!("msg-{}", i)
            } else {
                e.id.clone()
            };
            messages.push(TranscriptMessage {
                id,
                role: e.role.clone(),
                content: e.content.clone(),
            });
        }
    }
    messages
}
