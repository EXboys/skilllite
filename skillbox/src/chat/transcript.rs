//! Transcript store: *.jsonl append-only, tree structure.
//!
//! Entry types: message, custom_message, custom, compaction, branch_summary.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptEntry {
    Session {
        id: String,
        cwd: Option<String>,
        timestamp: String,
    },
    Message {
        id: String,
        parent_id: Option<String>,
        role: String,
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<serde_json::Value>,
    },
    CustomMessage {
        id: String,
        parent_id: Option<String>,
        #[serde(flatten)]
        data: serde_json::Value,
    },
    Custom {
        id: String,
        parent_id: Option<String>,
        kind: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },
    Compaction {
        id: String,
        parent_id: Option<String>,
        first_kept_entry_id: String,
        tokens_before: u64,
        summary: Option<String>,
    },
    BranchSummary {
        id: String,
        parent_id: Option<String>,
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

impl TranscriptEntry {
    pub fn entry_id(&self) -> Option<&str> {
        match self {
            Self::Session { id, .. } => Some(id),
            Self::Message { id, .. } => Some(id),
            Self::CustomMessage { id, .. } => Some(id),
            Self::Custom { id, .. } => Some(id),
            Self::Compaction { id, .. } => Some(id),
            Self::BranchSummary { id, .. } => Some(id),
        }
    }

    pub fn parent_id(&self) -> Option<&str> {
        match self {
            Self::Message { parent_id, .. } => parent_id.as_deref(),
            Self::CustomMessage { parent_id, .. } => parent_id.as_deref(),
            Self::Custom { parent_id, .. } => parent_id.as_deref(),
            Self::Compaction { parent_id, .. } => parent_id.as_deref(),
            Self::BranchSummary { parent_id, .. } => parent_id.as_deref(),
            _ => None,
        }
    }
}

/// Append an entry to transcript file. Creates file and parent dir if needed.
pub fn append_entry(transcript_path: &Path, entry: &TranscriptEntry) -> Result<()> {
    if let Some(parent) = transcript_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path)
        .with_context(|| format!("Failed to open transcript: {}", transcript_path.display()))?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

/// Read all entries from transcript (for context building). Returns entries in order.
pub fn read_entries(transcript_path: &Path) -> Result<Vec<TranscriptEntry>> {
    if !transcript_path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(transcript_path)
        .with_context(|| format!("Failed to open transcript: {}", transcript_path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let entry: TranscriptEntry = serde_json::from_str(line)?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Ensure transcript has session header. Call once when creating new transcript.
pub fn ensure_session_header(
    transcript_path: &Path,
    session_id: &str,
    cwd: Option<&str>,
) -> Result<()> {
    if transcript_path.exists() {
        let entries = read_entries(transcript_path)?;
        if !entries.is_empty() {
            if let TranscriptEntry::Session { .. } = &entries[0] {
                return Ok(()); // already has header
            }
        }
    }
    let header = TranscriptEntry::Session {
        id: session_id.to_string(),
        cwd: cwd.map(|s| s.to_string()),
        timestamp: timestamp_now(),
    };
    append_entry(transcript_path, &header)
}

fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}
