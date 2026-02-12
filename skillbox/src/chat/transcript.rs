//! Transcript store: *.jsonl append-only, tree structure.
//!
//! Entry types: message, custom_message, custom, compaction, branch_summary.
//!
//! Time-based segmentation (aligned with OpenClaw): files are named
//! `{session_key}-YYYY-MM-DD.jsonl` so each day gets a new file. Legacy
//! `{session_key}.jsonl` without date is still supported for backward compat.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

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

/// Date string for today (YYYY-MM-DD), local timezone. Used for log segmentation.
fn date_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Path for session's transcript file. With date segmentation: `{session_key}-YYYY-MM-DD.jsonl`.
pub fn transcript_path_for_session(
    transcripts_dir: &Path,
    session_key: &str,
    date: Option<&str>,
) -> PathBuf {
    let date_str = date.map(|s| s.to_string()).unwrap_or_else(date_today);
    transcripts_dir.join(format!("{}-{}.jsonl", session_key, date_str))
}

/// Path for today's transcript file (used for append).
pub fn transcript_path_today(transcripts_dir: &Path, session_key: &str) -> PathBuf {
    transcript_path_for_session(transcripts_dir, session_key, None)
}

/// List all transcript files for a session, sorted by date (legacy first, then YYYY-MM-DD).
pub fn list_transcript_files(transcripts_dir: &Path, session_key: &str) -> Result<Vec<PathBuf>> {
    let legacy = transcripts_dir.join(format!("{}.jsonl", session_key));
    let mut files = Vec::new();
    if legacy.exists() {
        files.push(legacy);
    }
    if !transcripts_dir.exists() {
        return Ok(files);
    }
    let entries = std::fs::read_dir(transcripts_dir).with_context(|| {
        format!(
            "Failed to read transcripts dir: {}",
            transcripts_dir.display()
        )
    })?;
    for e in entries {
        let e = e?;
        let path = e.path();
        if let Some(name) = path.file_name() {
            let name = name.to_string_lossy();
            if name.starts_with(session_key) && name.ends_with(".jsonl") && name != format!("{}.jsonl", session_key) {
                files.push(path);
            }
        }
    }
    files.sort_by(|a, b| {
        let date_a = extract_date_from_path(a, session_key);
        let date_b = extract_date_from_path(b, session_key);
        date_a.cmp(&date_b)
    });
    Ok(files)
}

fn extract_date_from_path(path: &Path, session_key: &str) -> String {
    let name = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    if name == session_key {
        return "0000-00-00".to_string(); // legacy, treat as oldest
    }
    let prefix = format!("{}-", session_key);
    if name.starts_with(&prefix) {
        name.trim_start_matches(&prefix).to_string()
    } else {
        "0000-00-00".to_string()
    }
}

/// Read all entries from all transcript files for a session (merged in date order).
pub fn read_entries_for_session(
    transcripts_dir: &Path,
    session_key: &str,
) -> Result<Vec<TranscriptEntry>> {
    let paths = list_transcript_files(transcripts_dir, session_key)?;
    let mut all = Vec::new();
    for p in paths {
        let entries = read_entries(&p)?;
        all.extend(entries);
    }
    Ok(all)
}
