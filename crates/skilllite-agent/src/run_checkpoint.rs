//! A13: Run mode checkpoint — save/restore state for long-running tasks.
//!
//! Enables `skilllite run --resume` to continue from where a previous run left off.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::types::{ChatMessage, Task};

/// Checkpoint state for run mode. Persisted to ~/.skilllite/chat/run_checkpoints/latest.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunCheckpoint {
    pub run_id: String,
    pub goal: String,
    pub workspace: String,
    pub task_plan: Vec<Task>,
    pub messages: Vec<ChatMessage>,
    pub updated_at: String,
}

impl RunCheckpoint {
    pub fn new(goal: String, workspace: String, task_plan: Vec<Task>, messages: Vec<ChatMessage>) -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            goal,
            workspace,
            task_plan,
            messages,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Update with new state (preserves run_id).
    pub fn update(&mut self, task_plan: Vec<Task>, messages: Vec<ChatMessage>) {
        self.task_plan = task_plan;
        self.messages = messages;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

const CHECKPOINT_DIR: &str = "run_checkpoints";
const CHECKPOINT_FILE: &str = "latest.json";

/// Save checkpoint to chat_root/run_checkpoints/latest.json.
pub fn save_checkpoint(chat_root: &Path, checkpoint: &RunCheckpoint) -> Result<()> {
    let dir = chat_root.join(CHECKPOINT_DIR);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(CHECKPOINT_FILE);
    let content = serde_json::to_string_pretty(checkpoint)?;
    // Atomic write: write to .tmp then rename
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, &path)?;
    tracing::debug!("Run checkpoint saved to {}", path.display());
    Ok(())
}

/// Load checkpoint from chat_root/run_checkpoints/latest.json.
/// Returns None if no checkpoint exists or file is invalid.
pub fn load_checkpoint(chat_root: &Path) -> Result<Option<RunCheckpoint>> {
    let path = chat_root.join(CHECKPOINT_DIR).join(CHECKPOINT_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let checkpoint: RunCheckpoint = serde_json::from_str(&content)?;
    Ok(Some(checkpoint))
}

/// Build continuation message for resume. Injects context so the agent continues from checkpoint.
pub fn build_resume_message(checkpoint: &RunCheckpoint) -> String {
    let completed: Vec<String> = checkpoint
        .task_plan
        .iter()
        .filter(|t| t.completed)
        .map(|t| format!("  - [完成] {}", t.description))
        .collect();
    let remaining: Vec<String> = checkpoint
        .task_plan
        .iter()
        .filter(|t| !t.completed)
        .map(|t| format!("  - [待做] {}", t.description))
        .collect();

    let msg = format!(
        "[断点续跑] 继续执行以下目标:\n\n{}\n\n已完成:\n{}\n\n待完成:\n{}\n\n请从下一个待完成任务继续执行。",
        checkpoint.goal,
        if completed.is_empty() {
            "  (无)".to_string()
        } else {
            completed.join("\n")
        },
        if remaining.is_empty() {
            "  (无 — 请确认目标是否已完成)".to_string()
        } else {
            remaining.join("\n")
        }
    );
    msg
}
