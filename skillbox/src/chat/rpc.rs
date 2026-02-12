//! JSON-RPC handlers for chat feature.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;

use super::memory::{ensure_index, index_file, index_path, search_bm25};
use super::session::SessionStore;
use super::transcript::{append_entry, ensure_session_header, TranscriptEntry};
use super::workspace_root;

pub fn handle_session_create(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());

    let root = workspace_root(workspace_path)?;
    let sessions_path = root.join("sessions.json");

    let mut store = SessionStore::load(&sessions_path)?;
    let entry = store.create_or_get(session_key);
    let session_id = entry.session_id.clone();
    let session_key_out = entry.session_key.clone();
    let updated_at = entry.updated_at.clone();
    store.save(&sessions_path)?;

    Ok(json!({
        "session_id": session_id,
        "session_key": session_key_out,
        "updated_at": updated_at,
    }))
}

pub fn handle_session_get(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());

    let root = workspace_root(workspace_path)?;
    let store = SessionStore::load(&root.join("sessions.json"))?;

    let entry = store.get(session_key).context("Session not found")?;
    Ok(json!({
        "session_id": entry.session_id,
        "session_key": entry.session_key,
        "updated_at": entry.updated_at,
        "input_tokens": entry.input_tokens,
        "output_tokens": entry.output_tokens,
        "total_tokens": entry.total_tokens,
        "context_tokens": entry.context_tokens,
        "compaction_count": entry.compaction_count,
    }))
}

pub fn handle_session_update(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());

    let root = workspace_root(workspace_path)?;
    let sessions_path = root.join("sessions.json");
    let mut store = SessionStore::load(&sessions_path)?;

    store.update(session_key, |e| {
        if let Some(v) = p.get("input_tokens").and_then(|v| v.as_u64()) {
            e.input_tokens = v;
        }
        if let Some(v) = p.get("output_tokens").and_then(|v| v.as_u64()) {
            e.output_tokens = v;
        }
        if let Some(v) = p.get("total_tokens").and_then(|v| v.as_u64()) {
            e.total_tokens = v;
        }
        if let Some(v) = p.get("context_tokens").and_then(|v| v.as_u64()) {
            e.context_tokens = v;
        }
        if let Some(v) = p.get("compaction_count").and_then(|v| v.as_u64()) {
            e.compaction_count = v as u32;
        }
    })?;
    store.save(&sessions_path)?;

    Ok(json!({"ok": true}))
}

pub fn handle_transcript_append(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());
    let entry_json = p.get("entry").context("entry required")?;

    let root = workspace_root(workspace_path)?;
    let transcript_path = root.join("transcripts").join(format!("{}.jsonl", session_key));

    // Accept flexible entry format - try structured first, else append raw line
    let entry: TranscriptEntry = match serde_json::from_value(entry_json.clone()) {
        Ok(e) => e,
        Err(_) => {
            // Raw append: write the entry as a single JSON line
            if let Some(parent) = transcript_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&transcript_path)?;
            use std::io::Write;
            writeln!(file, "{}", entry_json)?;
            return Ok(json!({"ok": true}));
        }
    };
    append_entry(&transcript_path, &entry)?;

    Ok(json!({
        "ok": true,
        "entry_id": entry.entry_id(),
    }))
}

pub fn handle_transcript_read(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());

    let root = workspace_root(workspace_path)?;
    let transcript_path = root.join("transcripts").join(format!("{}.jsonl", session_key));

    if !transcript_path.exists() {
        return Ok(json!([]));
    }

    let content = fs::read_to_string(&transcript_path)?;
    let arr: Vec<Value> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            serde_json::from_str(line).ok()
        })
        .collect();
    Ok(json!(arr))
}

pub fn handle_transcript_ensure(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let session_key = p.get("session_key").and_then(|v| v.as_str()).context("session_key required")?;
    let session_id = p.get("session_id").and_then(|v| v.as_str()).context("session_id required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());
    let cwd = p.get("cwd").and_then(|v| v.as_str());

    let root = workspace_root(workspace_path)?;
    let transcript_path = root.join("transcripts").join(format!("{}.jsonl", session_key));

    ensure_session_header(&transcript_path, session_id, cwd)?;
    Ok(json!({"ok": true}))
}

pub fn handle_memory_write(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let rel_path = p.get("rel_path").and_then(|v| v.as_str()).context("rel_path required")?;
    let content = p.get("content").and_then(|v| v.as_str()).context("content required")?;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());
    let append = p.get("append").and_then(|v| v.as_bool()).unwrap_or(false);
    let agent_id = p.get("agent_id").and_then(|v| v.as_str()).unwrap_or("default");

    let root = workspace_root(workspace_path)?;
    let full_path = root.join("memory").join(rel_path);

    if rel_path.is_empty() || rel_path.contains("..") || rel_path.starts_with('/') {
        anyhow::bail!("Invalid rel_path: must be relative, without ..");
    }

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if append {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&full_path)?;
        file.write_all(content.as_bytes())?;
    } else {
        fs::write(&full_path, content)?;
    }

    // Index into FTS5
    let index_path = index_path(&root, agent_id);
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let conn = rusqlite::Connection::open(&index_path)?;
    ensure_index(&conn)?;
    let file_content = fs::read_to_string(&full_path).unwrap_or_default();
    index_file(&conn, rel_path, &file_content)?;

    Ok(json!({"ok": true, "path": rel_path}))
}

pub fn handle_memory_search(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let query = p.get("query").and_then(|v| v.as_str()).context("query required")?;
    let limit = p.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as i64;
    let workspace_path = p.get("workspace_path").and_then(|v| v.as_str());
    let agent_id = p.get("agent_id").and_then(|v| v.as_str()).unwrap_or("default");

    let root = workspace_root(workspace_path)?;
    let idx_path = index_path(&root, agent_id);

    if !idx_path.exists() {
        return Ok(json!([]));
    }

    let conn = rusqlite::Connection::open(&idx_path)?;
    let hits = search_bm25(&conn, query, limit)?;

    let arr: Vec<Value> = hits
        .iter()
        .map(|h| {
            json!({
                "path": h.path,
                "chunk_index": h.chunk_index,
                "content": h.content,
                "score": h.score,
            })
        })
        .collect();
    Ok(json!(arr))
}

pub fn handle_token_count(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let text = p.get("text").and_then(|v| v.as_str()).context("text required")?;

    // Approximate: ~4 chars per token
    let count = (text.len() as f64 / 4.0).ceil() as u64;
    Ok(json!({"tokens": count}))
}
