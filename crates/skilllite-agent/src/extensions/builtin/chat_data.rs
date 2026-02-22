//! Chat data read tools: chat_history, chat_plan, update_task_plan definition.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::types::{ToolDefinition, FunctionDef};

// ─── Tool definitions ───────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "chat_history".to_string(),
                description: "Read chat history from the session. Use when the user asks to view, summarize, or analyze past conversations. Returns messages in chronological order. The transcript may contain [compaction] entries — these are summaries from /compact (history compression). To analyze /compact effect, read the transcript and find the [compaction] block.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "date": {
                            "type": "string",
                            "description": "Optional. Date to read (YYYY-MM-DD or YYYYMMDD). If omitted, returns all available history. For 昨天/yesterday, use (today - 1 day). Check system prompt for current date."
                        },
                        "session_key": {
                            "type": "string",
                            "description": "Optional. Use the session_key from system prompt (default: 'default'). For current interactive chat, use that value; do NOT use 'memory' — that is a different concept."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "chat_plan".to_string(),
                description: "Read the task plan for a session. Use when the user asks about today's plan, task status, or what was planned.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "date": {
                            "type": "string",
                            "description": "Optional. Date (YYYY-MM-DD or YYYYMMDD). Default: today."
                        },
                        "session_key": {
                            "type": "string",
                            "description": "Optional. Session key (default: 'default')."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "update_task_plan".to_string(),
                description: "Revise the task plan when current tasks are unusable (e.g. chat_history returned irrelevant data for a city comparison). Call with the new task list. Use when: (1) a task's result is clearly not useful for the user's goal; (2) the plan was wrong (e.g. used chat_history for place comparison). Pass `tasks` array with id, description, tool_hint, completed.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "tasks": {
                            "type": "array",
                            "description": "New task list. Each task: {id, description, tool_hint?, completed: false}",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {"type": "number"},
                                    "description": {"type": "string"},
                                    "tool_hint": {"type": "string"},
                                    "completed": {"type": "boolean", "default": false}
                                },
                                "required": ["id", "description"]
                            }
                        },
                        "reason": {
                            "type": "string",
                            "description": "Brief reason for the plan revision (e.g. chat_history had no relevant city data)"
                        }
                    },
                    "required": ["tasks"]
                }),
            },
        },
    ]
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn chat_data_root() -> Result<PathBuf> {
    let root = skilllite_executor::workspace_root(None)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".skilllite"));
    Ok(root.join("chat"))
}

fn normalize_date(date: &str) -> String {
    let s = date.trim().replace('-', "");
    if s.len() == 8 {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        date.to_string()
    }
}

// ─── Execution ──────────────────────────────────────────────────────────────

pub(super) fn execute_chat_history(args: &Value) -> Result<String> {
    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let date: Option<String> = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(|s| normalize_date(s));

    let chat_root = chat_data_root()?;
    let transcripts_dir = chat_root.join("transcripts");

    if !transcripts_dir.exists() {
        return Ok("No chat history found. Transcripts directory does not exist.".to_string());
    }

    let entries = if let Some(ref d) = date {
        let path = skilllite_executor::transcript::transcript_path_for_session(
            &transcripts_dir,
            session_key,
            Some(d),
        );
        if path.exists() {
            skilllite_executor::transcript::read_entries(&path)?
        } else {
            return Ok(format!(
                "No chat history found for session '{}' on date {}.",
                session_key, d
            ));
        }
    } else {
        skilllite_executor::transcript::read_entries_for_session(&transcripts_dir, session_key)?
    };

    if entries.is_empty() {
        return Ok(format!(
            "No chat history found for session '{}'.",
            session_key
        ));
    }

    use skilllite_executor::transcript::TranscriptEntry;
    let mut lines = Vec::new();
    for entry in entries {
        match entry {
            TranscriptEntry::Session { .. } => {}
            TranscriptEntry::Message { role, content, .. } => {
                if let Some(c) = content {
                    lines.push(format!("[{}] {}", role, c.trim()));
                }
            }
            TranscriptEntry::Compaction { summary, .. } => {
                if let Some(s) = summary {
                    lines.push(format!("[compaction] {}", s));
                }
            }
            _ => {}
        }
    }
    Ok(lines.join("\n\n"))
}

pub(super) fn execute_chat_plan(args: &Value) -> Result<String> {
    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let date_str = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(|s| normalize_date(s))
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let chat_root = chat_data_root()?;
    let plans_dir = chat_root.join("plans");
    let plan_path = plans_dir.join(format!("{}-{}.json", session_key, date_str));

    if !plan_path.exists() {
        return Ok(format!(
            "No plan found for session '{}' on date {}.",
            session_key, date_str
        ));
    }

    let content = std::fs::read_to_string(&plan_path)
        .with_context(|| format!("Failed to read plan: {}", plan_path.display()))?;
    let plan: Value = serde_json::from_str(&content)
        .with_context(|| "Invalid plan JSON")?;

    let task = plan.get("task").and_then(|v| v.as_str()).unwrap_or("");
    let empty: Vec<Value> = vec![];
    let steps = plan.get("steps").and_then(|v| v.as_array()).unwrap_or(&empty);

    let mut lines = vec![format!("Task: {}", task), "Steps:".to_string()];
    for (i, step) in steps.iter().enumerate() {
        let desc = step.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let status = step.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
        lines.push(format!("  {}. [{}] {}", i + 1, status, desc));
    }
    Ok(lines.join("\n"))
}
