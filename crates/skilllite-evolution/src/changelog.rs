//! Append-only changelog under prompts/_versions/.

use std::path::Path;

use crate::snapshots::versions_dir;
use crate::Result;

// ─── Changelog ───────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct ChangelogEntry {
    txn_id: String,
    ts: String,
    files: Vec<String>,
    changes: Vec<ChangeDetail>,
    reason: String,
}

#[derive(serde::Serialize)]
struct ChangeDetail {
    #[serde(rename = "type")]
    change_type: String,
    id: String,
}

pub fn append_changelog(
    chat_root: &Path,
    txn_id: &str,
    files: &[String],
    changes: &[(String, String)],
    reason: &str,
) -> Result<()> {
    let vdir = versions_dir(chat_root);
    std::fs::create_dir_all(&vdir)?;
    let path = vdir.join("changelog.jsonl");

    let entry = ChangelogEntry {
        txn_id: txn_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        files: files.to_vec(),
        changes: changes
            .iter()
            .map(|(t, id)| ChangeDetail {
                change_type: t.clone(),
                id: id.clone(),
            })
            .collect(),
        reason: reason.to_string(),
    };

    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}
