//! sessions.json 与会话 CRUD。

use serde_json::json;
use std::path::PathBuf;

use super::paths::skilllite_chat_root;
use super::transcript::list_transcript_paths;

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
                .unwrap_or(if key == "default" {
                    "默认会话"
                } else {
                    key
                })
                .to_string();
            let updated_at = val
                .get("updated_at")
                .and_then(|u| u.as_str())
                .unwrap_or("0")
                .to_string();
            let message_preview = get_last_user_message_from_transcripts(&transcripts_dir, key);
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

#[cfg(test)]
mod tests {
    use super::list_transcript_paths;

    #[test]
    fn list_transcript_paths_sorts_dated_files() {
        let tmp =
            std::env::temp_dir().join(format!("skilllite-transcript-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("s1-2026-01-01.jsonl"), "").unwrap();
        std::fs::write(tmp.join("s1-2026-01-03.jsonl"), "").unwrap();
        std::fs::write(tmp.join("s1-2026-01-02.jsonl"), "").unwrap();
        let paths = list_transcript_paths(&tmp, "s1");
        assert_eq!(paths.len(), 3);
        let names: Vec<_> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                "s1-2026-01-01.jsonl",
                "s1-2026-01-02.jsonl",
                "s1-2026-01-03.jsonl",
            ]
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn list_transcript_paths_includes_legacy_file() {
        let tmp = std::env::temp_dir().join(format!(
            "skilllite-transcript-legacy-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("default.jsonl"), "").unwrap();
        let paths = list_transcript_paths(&tmp, "default");
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("default.jsonl"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
