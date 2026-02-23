//! Plan store: append-only jsonl, similar to transcript.
//!
//! Each plan is appended as a JSON line. Supports reading latest plan.
//! Backward compatible: can still read legacy single .json files.

use anyhow::{Context, Result};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

fn date_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Path for plan jsonl file: plans/{session_key}-{date}.jsonl
pub fn plan_path_jsonl(plans_dir: &Path, session_key: &str, date: Option<&str>) -> PathBuf {
    let date_str = date.map(|s| s.to_string()).unwrap_or_else(date_today);
    plans_dir.join(format!("{}-{}.jsonl", session_key, date_str))
}

/// Legacy path: plans/{session_key}-{date}.json (single file, overwrite)
pub fn plan_path_legacy(plans_dir: &Path, session_key: &str, date: Option<&str>) -> PathBuf {
    let date_str = date.map(|s| s.to_string()).unwrap_or_else(date_today);
    plans_dir.join(format!("{}-{}.json", session_key, date_str))
}

/// Append a plan entry to the jsonl file.
pub fn append_plan(plans_dir: &Path, session_key: &str, plan_json: &Value) -> Result<()> {
    let path = plan_path_jsonl(plans_dir, session_key, None);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open plan: {}", path.display()))?;
    let line = serde_json::to_string(plan_json)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

/// Read plan entries from jsonl. Returns all entries in order.
fn read_plan_entries(plans_dir: &Path, session_key: &str, date: Option<&str>) -> Result<Vec<Value>> {
    let path = plan_path_jsonl(plans_dir, session_key, date);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open plan: {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line)?;
        entries.push(v);
    }
    Ok(entries)
}

/// Read the latest plan. Tries jsonl first (last entry), then legacy .json.
pub fn read_latest_plan(
    plans_dir: &Path,
    session_key: &str,
    date: Option<&str>,
) -> Result<Option<Value>> {
    let entries = read_plan_entries(plans_dir, session_key, date)?;
    if let Some(last) = entries.last() {
        return Ok(Some(last.clone()));
    }
    // Fallback: legacy single .json file
    let legacy_path = plan_path_legacy(plans_dir, session_key, date);
    if legacy_path.exists() {
        let content = std::fs::read_to_string(&legacy_path)?;
        let plan: Value = serde_json::from_str(&content)?;
        return Ok(Some(plan));
    }
    Ok(None)
}

/// List all plan files for a session (for UI / history browsing).
pub fn list_plan_files(plans_dir: &Path, session_key: &str) -> Result<Vec<PathBuf>> {
    if !plans_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(plans_dir)
        .with_context(|| format!("Failed to read plans dir: {}", plans_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map_or(false, |e| e == "jsonl" || e == "json")
                && p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map_or(false, |n| n.starts_with(session_key))
        })
        .collect();
    files.sort_by(|a, b| {
        std::fs::metadata(a)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &std::fs::metadata(b)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });
    Ok(files)
}
