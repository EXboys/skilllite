//! 会话 transcript 加载与 clear-session。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::paths::{find_project_root, load_dotenv_for_child, skilllite_chat_root};

/// Single message entry for frontend display.
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
struct TranscriptEntryRaw {
    ty: String,
    id: String,
    role: String,
    content: String,
    summary: Option<String>,
    name: Option<String>,
    is_error: Option<bool>,
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
