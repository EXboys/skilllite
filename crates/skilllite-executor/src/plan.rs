//! Plan store: append-only jsonl, similar to transcript.
//!
//! Each plan is appended as a JSON line. Supports reading latest plan.
//! Backward compatible: can still read legacy single .json files.

use crate::error::{Error, Result};
use anyhow::Context;
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

fn date_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Path for plan jsonl file: plans/{session_key}-{date}.jsonl
pub fn plan_path_jsonl(plans_dir: &Path, session_key: &str, date: Option<&str>) -> Result<PathBuf> {
    let date_str = date.map(|s| s.to_string()).unwrap_or_else(date_today);
    let file_name = format!("{}-{}.jsonl", session_key, date_str);
    skilllite_core::path_validation::session_file_under_dir(plans_dir, session_key, &file_name)
        .map_err(|e| Error::validation(e.to_string()))
}

/// Legacy path: plans/{session_key}-{date}.json (single file, overwrite)
pub fn plan_path_legacy(
    plans_dir: &Path,
    session_key: &str,
    date: Option<&str>,
) -> Result<PathBuf> {
    let date_str = date.map(|s| s.to_string()).unwrap_or_else(date_today);
    let file_name = format!("{}-{}.json", session_key, date_str);
    skilllite_core::path_validation::session_file_under_dir(plans_dir, session_key, &file_name)
        .map_err(|e| Error::validation(e.to_string()))
}

/// Append a plan entry to the jsonl file.
pub fn append_plan(plans_dir: &Path, session_key: &str, plan_json: &Value) -> Result<()> {
    let path = plan_path_jsonl(plans_dir, session_key, None)?;
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
fn read_plan_entries(
    plans_dir: &Path,
    session_key: &str,
    date: Option<&str>,
) -> Result<Vec<Value>> {
    let path = plan_path_jsonl(plans_dir, session_key, date)?;
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
    let legacy_path = plan_path_legacy(plans_dir, session_key, date)?;
    if legacy_path.exists() {
        let content = skilllite_fs::read_file(&legacy_path)
            .map_err(|e| crate::error::Error::Other(e.into()))?;
        let plan: Value = serde_json::from_str(&content)?;
        return Ok(Some(plan));
    }
    Ok(None)
}

/// List all plan files for a session (for UI / history browsing).
pub fn list_plan_files(plans_dir: &Path, session_key: &str) -> Result<Vec<PathBuf>> {
    skilllite_core::path_validation::validate_session_key(session_key)
        .map_err(|e| Error::validation(e.to_string()))?;
    if !plans_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = skilllite_fs::read_dir(plans_dir)
        .with_context(|| format!("Failed to read plans dir: {}", plans_dir.display()))?
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "jsonl" || e == "json")
                && p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|n| n.starts_with(session_key))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_path_rejects_path_escaping_session_keys() {
        let dir = PathBuf::from("/tmp/skilllite-chat/plans");
        for key in ["/tmp/evil", "../evil", "foo/bar", "C:/Windows/Temp"] {
            let err = plan_path_jsonl(&dir, key, Some("2026-07-28")).unwrap_err();
            assert!(
                err.to_string().contains("invalid session key"),
                "expected rejection for {key:?}, got {err}"
            );
        }
    }

    #[test]
    fn append_plan_rejects_absolute_session_key_before_write() {
        let tmp = tempfile::tempdir().unwrap();
        let plans = tmp.path().join("plans");
        std::fs::create_dir_all(&plans).unwrap();
        let outside = tmp.path().join("pwned");
        let key = outside.to_string_lossy();
        let err = append_plan(&plans, key.as_ref(), &serde_json::json!({"ok": true})).unwrap_err();
        assert!(err.to_string().contains("invalid session key"));
        assert!(!outside.exists());
        assert!(std::fs::read_dir(&plans).unwrap().next().is_none());
    }
}
