//! Chat session: persistent conversation with transcript and memory.
//!
//! Ported from Python `ChatSession`. Directly calls executor module
//! (same process, no IPC). Handles transcript persistence, auto-compaction,
//! and memory integration.

use anyhow::Result;
use std::path::PathBuf;

use skilllite_executor::{session, transcript};

use super::agent_loop;
use super::extensions;
use super::llm::LlmClient;
use super::skills::LoadedSkill;
use super::types::*;

// Compaction threshold/keep are configurable via types::get_compaction_threshold()
// and types::get_compaction_keep_recent() (SKILLLITE_COMPACTION_* env vars).

/// Persistent chat session.
///
/// Storage layout (matching Python SDK, stored in `~/.skilllite/`):
///   sessions.json            — session metadata
///   transcripts/{key}-{date}.jsonl — append-only transcript
pub struct ChatSession {
    config: AgentConfig,
    session_key: String,
    session_id: Option<String>,
    /// Data root for sessions/transcripts/memory — always `~/.skilllite/`.
    /// NOT the user's workspace directory.
    data_root: PathBuf,
    skills: Vec<LoadedSkill>,
}

impl ChatSession {
    pub fn new(config: AgentConfig, session_key: &str, skills: Vec<LoadedSkill>) -> Self {
        // Data storage goes to ~/.skilllite/chat/ (matching Python SDK layout).
        // ~/.skilllite/ is the root for all skilllite data:
        //   bin/          — binary
        //   chat/         — sessions, transcripts, plans, memory, output
        let data_root = skilllite_executor::workspace_root(None)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".skilllite")
            })
            .join("chat");
        Self {
            config,
            session_key: session_key.to_string(),
            session_id: None,
            data_root,
            skills,
        }
    }

    /// Ensure session and transcript exist, return session_id.
    fn ensure_session(&mut self) -> Result<String> {
        if let Some(ref id) = self.session_id {
            return Ok(id.clone());
        }

        // Ensure data_root directory exists
        if !self.data_root.exists() {
            std::fs::create_dir_all(&self.data_root)?;
        }

        let sessions_path = self.data_root.join("sessions.json");
        let mut store = session::SessionStore::load(&sessions_path)?;
        let entry = store.create_or_get(&self.session_key);
        let session_id = entry.session_id.clone();
        store.save(&sessions_path)?;

        // Ensure transcript
        let transcripts_dir = self.data_root.join("transcripts");
        let t_path = transcript::transcript_path_today(&transcripts_dir, &self.session_key);
        transcript::ensure_session_header(&t_path, &session_id, Some(&self.config.workspace))?;

        self.session_id = Some(session_id.clone());
        Ok(session_id)
    }

    /// Read transcript entries and convert to ChatMessages.
    fn read_history(&self) -> Result<Vec<ChatMessage>> {
        let transcripts_dir = self.data_root.join("transcripts");
        let entries = transcript::read_entries_for_session(&transcripts_dir, &self.session_key)?;

        let mut messages = Vec::new();
        let mut use_from_compaction = false;
        let mut compaction_summary: Option<String> = None;

        // Check for compaction — if present, use summary + entries after it
        for entry in entries.iter().rev() {
            if let transcript::TranscriptEntry::Compaction { summary, .. } = entry {
                use_from_compaction = true;
                compaction_summary = summary.clone();
                break;
            }
        }

        if use_from_compaction {
            // Add compaction summary as system context
            if let Some(summary) = compaction_summary {
                messages.push(ChatMessage::system(&format!(
                    "[Previous conversation summary]\n{}",
                    summary
                )));
            }

            // Find the compaction entry and take entries after it
            let mut past_compaction = false;
            for entry in &entries {
                if let transcript::TranscriptEntry::Compaction { .. } = entry {
                    past_compaction = true;
                    continue;
                }
                if past_compaction {
                    if let Some(msg) = transcript_entry_to_message(entry) {
                        messages.push(msg);
                    }
                }
            }
        } else {
            // No compaction, use all message entries
            for entry in &entries {
                if let Some(msg) = transcript_entry_to_message(entry) {
                    messages.push(msg);
                }
            }
        }

        Ok(messages)
    }

    /// Run one conversation turn.
    pub async fn run_turn(
        &mut self,
        user_message: &str,
        event_sink: &mut dyn EventSink,
    ) -> Result<String> {
        let _session_id = self.ensure_session()?;

        // Read history from transcript
        let history = self.read_history()?;
        if !history.is_empty() {
            tracing::debug!(
                session_key = %self.session_key,
                history_len = history.len(),
                "Loaded conversation history from transcript"
            );
        }

        // Check if compaction is needed
        let threshold = get_compaction_threshold();
        let mut history = if history.len() >= threshold {
            self.compact_history(history).await?
        } else {
            history
        };

        // Build memory context (if enabled) — inject relevant memories as system context
        if self.config.enable_memory {
            let workspace = std::path::Path::new(&self.config.workspace);
            if let Some(mem_ctx) = extensions::build_memory_context(workspace, "default", user_message) {
                history.push(ChatMessage::system(&mem_ctx));
            }
        }

        // Append user message to transcript
        self.append_message("user", user_message)?;

        event_sink.on_turn_start();

        // Run the agent loop
        let result = agent_loop::run_agent_loop(
            &self.config,
            history,
            user_message,
            &self.skills,
            event_sink,
            Some(&self.session_key),
        )
        .await?;

        // Persist task plan to plans/ directory (if non-empty)
        if !result.task_plan.is_empty() {
            if let Err(e) = self.persist_plan(user_message, &result.task_plan) {
                tracing::warn!("Failed to persist task plan: {}", e);
            }
        }

        // Append assistant response to transcript
        self.append_message("assistant", &result.response)?;

        Ok(result.response)
    }

    /// Append a message entry to the transcript.
    fn append_message(&self, role: &str, content: &str) -> Result<()> {
        let transcripts_dir = self.data_root.join("transcripts");
        let t_path = transcript::transcript_path_today(&transcripts_dir, &self.session_key);
        let entry = transcript::TranscriptEntry::Message {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            role: role.to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
        };
        transcript::append_entry(&t_path, &entry)
    }

    /// Persist the task plan to plans/{session_key}-{date}.json.
    /// Matches the format used by the RPC `plan_write` handler.
    fn persist_plan(&self, user_message: &str, tasks: &[super::types::Task]) -> Result<()> {
        let plans_dir = self.data_root.join("plans");
        if !plans_dir.exists() {
            std::fs::create_dir_all(&plans_dir)?;
        }

        let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
        let plan_path = plans_dir.join(format!("{}-{}.json", self.session_key, date_str));

        // Convert tasks to plan steps with status
        let mut steps = Vec::with_capacity(tasks.len());
        let mut current_step_id: u32 = 0;
        let mut found_running = false;
        for task in tasks {
            let status = if task.completed {
                "completed"
            } else if !found_running {
                found_running = true;
                current_step_id = task.id;
                "running"
            } else {
                "pending"
            };
            steps.push(serde_json::json!({
                "id": task.id,
                "description": task.description,
                "tool_hint": task.tool_hint,
                "status": status,
            }));
        }
        if current_step_id == 0 {
            if let Some(last) = tasks.last() {
                current_step_id = last.id;
            }
        }

        let plan_json = serde_json::json!({
            "session_key": self.session_key,
            "task": user_message,
            "steps": steps,
            "current_step_id": current_step_id,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });

        let pretty = serde_json::to_string_pretty(&plan_json)?;
        std::fs::write(&plan_path, pretty)?;
        tracing::info!("Task plan persisted to {}", plan_path.display());
        Ok(())
    }

    /// Compact old messages: summarize via LLM, write compaction entry.
    /// Ported from Python `_check_and_compact`.
    async fn compact_history(
        &mut self,
        history: Vec<ChatMessage>,
    ) -> Result<Vec<ChatMessage>> {
        let threshold = get_compaction_threshold();
        if history.len() < threshold {
            return Ok(history);
        }
        self.compact_history_inner(history, threshold).await
    }

    /// Inner compaction logic. `min_threshold`: use 0 for force_compact to bypass.
    async fn compact_history_inner(
        &mut self,
        history: Vec<ChatMessage>,
        min_threshold: usize,
    ) -> Result<Vec<ChatMessage>> {
        let keep_count = get_compaction_keep_recent();
        if history.len() < min_threshold || history.len() <= keep_count {
            return Ok(history);
        }

        let split_point = history.len().saturating_sub(keep_count);
        let old_messages = &history[..split_point];
        let recent_messages = &history[split_point..];

        // Build summary of old messages via LLM
        let client = LlmClient::new(&self.config.api_base, &self.config.api_key);
        let summary_prompt = format!(
            "Please summarize the following conversation concisely, preserving key context, decisions, and results:\n\n{}",
            old_messages
                .iter()
                .filter_map(|m| {
                    let content = m.content.as_deref().unwrap_or("");
                    if content.is_empty() { None }
                    else { Some(format!("[{}] {}", m.role, content)) }
                })
                .collect::<Vec<_>>()
                .join("\n")
        );

        let summary = match client
            .chat_completion(
                &self.config.model,
                &[ChatMessage::user(&summary_prompt)],
                None,
                Some(0.3),
            )
            .await
        {
            Ok(resp) => resp
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_else(|| "[Compaction summary unavailable]".to_string()),
            Err(e) => {
                tracing::warn!("Compaction summary failed: {}, keeping all messages", e);
                return Ok(history);
            }
        };

        // Write compaction entry to transcript
        let transcripts_dir = self.data_root.join("transcripts");
        let t_path = transcript::transcript_path_today(&transcripts_dir, &self.session_key);
        let compaction_entry = transcript::TranscriptEntry::Compaction {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            first_kept_entry_id: String::new(),
            tokens_before: (old_messages.len() * 100) as u64, // rough estimate
            summary: Some(summary.clone()),
        };
        transcript::append_entry(&t_path, &compaction_entry)?;

        // Update session compaction count
        let sessions_path = self.data_root.join("sessions.json");
        if let Ok(mut store) = session::SessionStore::load(&sessions_path) {
            if let Some(entry) = store.sessions.get_mut(&self.session_key) {
                entry.compaction_count += 1;
                entry.updated_at = chrono::Utc::now().to_rfc3339();
                let _ = store.save(&sessions_path);
            }
        }

        // Return summary + recent messages
        let mut result = Vec::new();
        result.push(ChatMessage::system(&format!(
            "[Previous conversation summary]\n{}",
            summary
        )));
        result.extend(recent_messages.to_vec());

        Ok(result)
    }

    /// Force compaction: summarize history via LLM regardless of threshold.
    /// Returns true if compaction was performed, false if history was too short.
    pub async fn force_compact(&mut self) -> Result<bool> {
        let _ = self.ensure_session()?;
        let history = self.read_history()?;
        let keep_count = get_compaction_keep_recent();
        if history.len() <= keep_count {
            return Ok(false);
        }
        let _ = self.compact_history_inner(history, 0).await?;
        Ok(true)
    }

    /// Clear session: summarize conversation to memory, then reset.
    /// Phase 2: generates a summary of the conversation before clearing.
    pub async fn clear(&mut self) -> Result<()> {
        // If we have a session, summarize the conversation before clearing
        if self.session_id.is_some() {
            if let Ok(history) = self.read_history() {
                if !history.is_empty() {
                    let _ = self.summarize_for_memory(&history).await;
                }
            }
        }
        self.session_id = None;
        Ok(())
    }

    /// Summarize conversation history and write to memory.
    /// Called before clearing a session to preserve key context.
    async fn summarize_for_memory(&self, history: &[ChatMessage]) -> Result<()> {
        let client = LlmClient::new(&self.config.api_base, &self.config.api_key);

        let conversation: Vec<String> = history
            .iter()
            .filter_map(|m| {
                let content = m.content.as_deref().unwrap_or("");
                if content.is_empty() {
                    None
                } else {
                    Some(format!("[{}] {}", m.role, content))
                }
            })
            .collect();

        if conversation.is_empty() {
            return Ok(());
        }

        let summary_prompt = format!(
            "Please summarize this conversation concisely for long-term memory. \
             Preserve key decisions, results, file paths, and important context:\n\n{}",
            conversation.join("\n")
        );

        let summary = match client
            .chat_completion(
                &self.config.model,
                &[ChatMessage::user(&summary_prompt)],
                None,
                Some(0.3),
            )
            .await
        {
            Ok(resp) => resp
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default(),
            Err(e) => {
                tracing::warn!("Memory summarization failed: {}", e);
                return Ok(());
            }
        };

        if summary.is_empty() {
            return Ok(());
        }

        // Write summary as a compaction entry in the transcript
        let transcripts_dir = self.data_root.join("transcripts");
        let t_path = transcript::transcript_path_today(&transcripts_dir, &self.session_key);
        let entry = transcript::TranscriptEntry::Compaction {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            first_kept_entry_id: String::new(),
            tokens_before: 0,
            summary: Some(format!("[Session cleared — memory summary]\n{}", summary)),
        };
        transcript::append_entry(&t_path, &entry)?;

        tracing::info!("Session memory summary written to transcript");
        Ok(())
    }
}

/// Convert a transcript entry to a ChatMessage.
fn transcript_entry_to_message(entry: &transcript::TranscriptEntry) -> Option<ChatMessage> {
    match entry {
        transcript::TranscriptEntry::Message {
            role, content, ..
        } => Some(ChatMessage {
            role: role.clone(),
            content: content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }),
        transcript::TranscriptEntry::Compaction { summary, .. } => {
            summary.as_ref().map(|s| {
                ChatMessage::system(&format!("[Previous conversation summary]\n{}", s))
            })
        }
        _ => None,
    }
}
