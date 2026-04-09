//! 会话 transcript 加载与 clear-session。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::paths::{find_project_root, load_dotenv_for_child, skilllite_chat_root};

/// Image preview for transcript reload (data URL for `<img src>`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptImagePreview {
    pub media_type: String,
    pub preview_url: String,
}

/// Token totals for one agent turn (from transcript `message.llm_usage` on assistant rows).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptLlmUsagePayload {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub responses_with_usage: u32,
    pub responses_without_usage: u32,
}

/// Single message entry for frontend display.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TranscriptMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<TranscriptImagePreview>>,
    /// Desktop-only rows restored from `custom_message` (confirmation / clarification).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<serde_json::Value>,
    /// Present on restored `assistant` messages when the agent persisted turn usage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_usage: Option<TranscriptLlmUsagePayload>,
}

/// List transcript file paths for session, sorted by date (legacy first, then YYYY-MM-DD).
pub(crate) fn list_transcript_paths(transcripts_dir: &Path, session_key: &str) -> Vec<PathBuf> {
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

#[derive(Clone)]
struct TranscriptImageRaw {
    media_type: String,
    data_base64: String,
}

#[derive(Clone)]
struct TranscriptEntryRaw {
    ty: String,
    id: String,
    role: String,
    content: String,
    summary: Option<String>,
    name: Option<String>,
    is_error: Option<bool>,
    ui: Option<serde_json::Value>,
    images: Option<Vec<TranscriptImageRaw>>,
    llm_usage: Option<serde_json::Value>,
}

fn parse_llm_usage_payload(v: &serde_json::Value) -> Option<TranscriptLlmUsagePayload> {
    let p = v.get("prompt_tokens")?.as_u64()?;
    let c = v.get("completion_tokens")?.as_u64()?;
    let t = v.get("total_tokens")?.as_u64()?;
    let rw = v
        .get("responses_with_usage")
        .and_then(|x| x.as_u64())
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    let rwo = v
        .get("responses_without_usage")
        .and_then(|x| x.as_u64())
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    Some(TranscriptLlmUsagePayload {
        prompt_tokens: p,
        completion_tokens: c,
        total_tokens: t,
        responses_with_usage: rw,
        responses_without_usage: rwo,
    })
}

fn parse_message_images(v: &serde_json::Value) -> Option<Vec<TranscriptImageRaw>> {
    let arr = v.get("images")?.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for item in arr {
        let media_type = item
            .get("media_type")
            .and_then(|x| x.as_str())
            .unwrap_or("image/png")
            .trim()
            .to_string();
        let data_base64 = item
            .get("data_base64")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if data_base64.is_empty() {
            continue;
        }
        out.push(TranscriptImageRaw {
            media_type,
            data_base64,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

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
                let content = v
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let images = parse_message_images(&v);
                let llm_usage = v.get("llm_usage").cloned();
                entries.push(TranscriptEntryRaw {
                    ty,
                    id: v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string(),
                    role,
                    content,
                    summary: None,
                    name: None,
                    is_error: None,
                    ui: None,
                    images,
                    llm_usage,
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
                    ui: None,
                    images: None,
                    llm_usage: None,
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
                    ui: None,
                    images: None,
                    llm_usage: None,
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
                    ui: None,
                    images: None,
                    llm_usage: None,
                });
            } else if ty == "custom_message" {
                let ui_kind = v
                    .get("ui_kind")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if ui_kind == "confirmation" {
                    let id = v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut ui = serde_json::Map::new();
                    ui.insert("kind".into(), json!("confirmation"));
                    ui.insert(
                        "prompt".into(),
                        v.get("prompt").cloned().unwrap_or(json!("")),
                    );
                    if let Some(rt) = v.get("risk_tier") {
                        ui.insert("risk_tier".into(), rt.clone());
                    }
                    ui.insert(
                        "resolved".into(),
                        json!(v.get("resolved").and_then(|b| b.as_bool()).unwrap_or(true)),
                    );
                    ui.insert(
                        "approved".into(),
                        json!(v.get("approved").and_then(|b| b.as_bool()).unwrap_or(false)),
                    );
                    let ui = Value::Object(ui);
                    entries.push(TranscriptEntryRaw {
                        ty: "custom_message".to_string(),
                        id,
                        role: "skilllite_ui".to_string(),
                        content: String::new(),
                        summary: None,
                        name: None,
                        is_error: None,
                        ui: Some(ui),
                        images: None,
                        llm_usage: None,
                    });
                } else if ui_kind == "clarification" {
                    let id = v
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let suggestions = v
                        .get("suggestions")
                        .cloned()
                        .unwrap_or_else(|| json!([]));
                    let ui = json!({
                        "kind": "clarification",
                        "reason": v.get("reason").cloned().unwrap_or(json!("")),
                        "message": v.get("message").cloned().unwrap_or(json!("")),
                        "suggestions": suggestions,
                        "resolved": v.get("resolved").and_then(|b| b.as_bool()).unwrap_or(true),
                        "action": v.get("action").cloned().unwrap_or(json!("stop")),
                        "hint": v.get("hint").cloned().unwrap_or(json!(null)),
                    });
                    entries.push(TranscriptEntryRaw {
                        ty: "custom_message".to_string(),
                        id,
                        role: "skilllite_ui".to_string(),
                        content: String::new(),
                        summary: None,
                        name: None,
                        is_error: None,
                        ui: Some(ui),
                        images: None,
                        llm_usage: None,
                    });
                }
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
                ..Default::default()
            });
        }
    }
    for (i, e) in to_use.iter().enumerate() {
        let dominated_by_type = e.ty == "message"
            || e.ty == "tool_call"
            || e.ty == "tool_result"
            || e.ty == "custom_message";
        if !dominated_by_type {
            continue;
        }
        if e.ty == "custom_message" {
            let id = if e.id.is_empty() {
                format!("msg-{}", i)
            } else {
                e.id.clone()
            };
            messages.push(TranscriptMessage {
                id,
                role: "skilllite_ui".to_string(),
                content: String::new(),
                ui: e.ui.clone(),
                ..Default::default()
            });
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
        let images = e.images.as_ref().and_then(|imgs| {
            let v: Vec<TranscriptImagePreview> = imgs
                .iter()
                .map(|im| TranscriptImagePreview {
                    media_type: im.media_type.clone(),
                    preview_url: format!(
                        "data:{};base64,{}",
                        im.media_type.trim(),
                        im.data_base64.trim()
                    ),
                })
                .collect();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        });
        let llm_usage = e.llm_usage.as_ref().and_then(parse_llm_usage_payload);
        messages.push(TranscriptMessage {
            id,
            role: e.role.clone(),
            content: e.content.clone(),
            name: e.name.clone(),
            is_error: e.is_error,
            images,
            ui: None,
            llm_usage,
        });
    }
    messages
}

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
