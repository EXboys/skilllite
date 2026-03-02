//! Chat session: persistent conversation with transcript and memory.
//!
//! Ported from Python `ChatSession`. Directly calls executor module
//! (same process, no IPC). Handles transcript persistence, auto-compaction,
//! and memory integration.

use anyhow::Result;
use std::path::PathBuf;

use skilllite_executor::{
    memory as executor_memory,
    session,
    transcript,
};

use skilllite_core::config::env_keys::evolution as evo_env_keys;

use super::agent_loop;
use super::evolution;
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
    /// A9: handle for periodic evolution (every N minutes, does not reset on turn).
    periodic_evolution_handle: Option<tokio::task::JoinHandle<()>>,
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
        // EVO-2: Ensure seed prompt/rules data exists on disk.
        skilllite_evolution::seed::ensure_seed_data(&data_root);
        let mut session = Self {
            config,
            session_key: session_key.to_string(),
            session_id: None,
            data_root,
            skills,
            periodic_evolution_handle: None,
        };
        // A9: Start periodic evolution timer (runs every 30 min even when user is active)
        session.start_periodic_evolution_timer();
        session
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
    ) -> Result<AgentResult> {
        self.run_turn_inner(user_message, event_sink, None).await
    }

    /// A13: Run with overridden history (for --resume from checkpoint).
    pub async fn run_turn_with_history(
        &mut self,
        user_message: &str,
        event_sink: &mut dyn EventSink,
        history_override: Vec<ChatMessage>,
    ) -> Result<AgentResult> {
        self.run_turn_inner(user_message, event_sink, Some(history_override))
            .await
    }

    async fn run_turn_inner(
        &mut self,
        user_message: &str,
        event_sink: &mut dyn EventSink,
        history_override: Option<Vec<ChatMessage>>,
    ) -> Result<AgentResult> {
        let _session_id = self.ensure_session()?;

        // EVO-1: Classify previous turn's user feedback from this message.
        // The feedback is attributed to the PREVIOUS decision, not the current one.
        self.update_previous_feedback(user_message);

        // Read history from transcript (or use override for resume)
        let history = if let Some(h) = history_override {
            h
        } else {
            self.read_history()?
        };
        if !history.is_empty() {
            tracing::debug!(
                session_key = %self.session_key,
                history_len = history.len(),
                "Loaded conversation history from transcript"
            );
        }

        // Early memory flush: run when history approaches compaction (OpenClaw-style).
        // Lower SKILLLITE_MEMORY_FLUSH_THRESHOLD (default 12) = more frequent triggers.
        let flush_threshold = get_memory_flush_threshold();
        let compaction_threshold = get_compaction_threshold();
        if self.config.enable_memory
            && get_memory_flush_enabled()
            && history.len() >= flush_threshold
        {
            let sessions_path = self.data_root.join("sessions.json");
            if let Ok(store) = session::SessionStore::load(&sessions_path) {
                if let Some(entry) = store.get(&self.session_key) {
                    let next_compaction = entry.compaction_count + 1;
                    let need_flush = entry.memory_flush_compaction_count != Some(next_compaction);
                    if need_flush {
                        if let Err(e) = self.run_memory_flush_turn(&history).await {
                            tracing::warn!("Early memory flush failed: {}", e);
                        } else {
                            if let Ok(mut store) = session::SessionStore::load(&sessions_path) {
                                if let Some(se) = store.sessions.get_mut(&self.session_key) {
                                    se.memory_flush_compaction_count = Some(next_compaction);
                                    se.memory_flush_at = Some(chrono::Utc::now().to_rfc3339());
                                    let _ = store.save(&sessions_path);
                                }
                            }
                            tracing::debug!("Early memory flush completed (threshold={})", flush_threshold);
                        }
                    }
                }
            }
        }

        // Check if compaction is needed
        let mut history = if history.len() >= compaction_threshold {
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

        // EVO-1: Record execution decision (async-safe, <1ms with WAL).
        // Only record meaningful turns (at least 1 tool call).
        if result.feedback.total_tools >= 1 {
            self.record_decision(&result.feedback);
            // A9: Decision-count trigger — if unprocessed decisions >= threshold, spawn evolution
            self.maybe_trigger_evolution_by_decision_count();
        }

        Ok(result)
    }

    /// Graceful shutdown: flush evolution metrics, cancel evolution timers.
    pub fn shutdown(&mut self) {
        if let Some(handle) = self.periodic_evolution_handle.take() {
            handle.abort();
        }
        shutdown_evolution(&self.data_root);
    }

    // ─── A9: Periodic + decision-count evolution triggers ────────────────────

    /// Start periodic evolution timer (every 30 min). Does not reset on user turns.
    fn start_periodic_evolution_timer(&mut self) {
        if skilllite_evolution::EvolutionMode::from_env().is_disabled() {
            return;
        }
        let interval_secs: u64 = std::env::var(evo_env_keys::SKILLLITE_EVOLUTION_INTERVAL_SECS)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1800); // 30 min default
        let data_root = self.data_root.clone();
        let workspace = self.config.workspace.clone();
        let api_base = self.config.api_base.clone();
        let api_key = self.config.api_key.clone();
        let model = self.config.model.clone();
        self.periodic_evolution_handle = Some(spawn_periodic_evolution(
            data_root, workspace, api_base, api_key, model, interval_secs,
        ));
    }

    /// A9: If unprocessed decisions >= threshold, spawn evolution (runs even when user is active).
    fn maybe_trigger_evolution_by_decision_count(&self) {
        if skilllite_evolution::EvolutionMode::from_env().is_disabled() {
            return;
        }
        let threshold: i64 = std::env::var(evo_env_keys::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&self.data_root) else {
            return;
        };
        let Ok(count) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) else {
            return;
        };
        if count >= threshold {
            tracing::debug!("Decision-count trigger: {} unprocessed >= {}, spawning evolution", count, threshold);
            let data_root = self.data_root.clone();
            let workspace = self.config.workspace.clone();
            let api_base = self.config.api_base.clone();
            let api_key = self.config.api_key.clone();
            let model = self.config.model.clone();
            spawn_evolution_once(data_root, workspace, api_base, api_key, model);
        }
    }

    // ─── EVO-1: Feedback collection helpers ─────────────────────────────────

    /// Record an execution decision to the evolution DB.
    fn record_decision(&self, feedback: &ExecutionFeedback) {
        if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&self.data_root) {
            let input = evolution::execution_feedback_to_decision_input(feedback);
            if let Err(e) = skilllite_evolution::feedback::insert_decision(
                &conn,
                Some(&self.session_key),
                &input,
                evolution::to_evolution_feedback(FeedbackSignal::Neutral),
            ) {
                tracing::warn!("Failed to record evolution decision: {}", e);
            }
            let _ = skilllite_evolution::feedback::update_daily_metrics(&conn);
        }
    }

    /// Update the previous decision's feedback signal based on the current user message.
    fn update_previous_feedback(&self, user_message: &str) {
        let signal = classify_user_feedback(user_message);
        if signal == FeedbackSignal::Neutral {
            return;
        }
        if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&self.data_root) {
            if let Err(e) = skilllite_evolution::feedback::update_last_decision_feedback(
                &conn,
                &self.session_key,
                evolution::to_evolution_feedback(signal),
            ) {
                tracing::debug!("Failed to update previous feedback: {}", e);
            }
        }
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

    /// Persist the task plan to plans/{session_key}-{date}.jsonl (append).
    /// Each plan is appended, preserving history. OpenClaw-style.
    fn persist_plan(&self, user_message: &str, tasks: &[super::types::Task]) -> Result<()> {
        let plans_dir = self.data_root.join("plans");

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

        skilllite_executor::plan::append_plan(&plans_dir, &self.session_key, &plan_json)?;
        tracing::info!("Task plan appended to plans/{}", self.session_key);
        Ok(())
    }

    /// Compact old messages: summarize via LLM, write compaction entry.
    /// Before compaction, runs pre-compaction memory flush (OpenClaw-style) when enabled:
    /// a silent agent turn reminds the model to write durable memories to memory/YYYY-MM-DD.md.
    async fn compact_history(
        &mut self,
        history: Vec<ChatMessage>,
    ) -> Result<Vec<ChatMessage>> {
        let threshold = get_compaction_threshold();
        if history.len() < threshold {
            return Ok(history);
        }

        // Pre-compaction memory flush (OpenClaw-style): give model a chance to save to memory
        // before we summarize away the conversation. Runs once per compaction cycle.
        if self.config.enable_memory
            && get_memory_flush_enabled()
        {
            let sessions_path = self.data_root.join("sessions.json");
            if let Ok(store) = session::SessionStore::load(&sessions_path) {
                if let Some(entry) = store.get(&self.session_key) {
                    let next_compaction_count = entry.compaction_count + 1;
                    let need_flush = entry.memory_flush_compaction_count != Some(next_compaction_count);
                    if need_flush {
                        if let Err(e) = self.run_memory_flush_turn(&history).await {
                            tracing::warn!("Memory flush failed (continuing with compaction): {}", e);
                        } else {
                            if let Ok(mut store) = session::SessionStore::load(&sessions_path) {
                                if let Some(session_entry) = store.sessions.get_mut(&self.session_key) {
                                    session_entry.memory_flush_compaction_count = Some(next_compaction_count);
                                    session_entry.memory_flush_at = Some(chrono::Utc::now().to_rfc3339());
                                    let _ = store.save(&sessions_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.compact_history_inner(history, threshold).await
    }

    /// Run a silent agent turn to remind the model to write durable memories before compaction.
    /// OpenClaw-style: system + user prompt, model may call memory_write, we don't show/output.
    async fn run_memory_flush_turn(&self, history: &[ChatMessage]) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let memory_flush_reminder = format!(
            "Session nearing compaction. Store durable memories now. \
             Use memory_write to save key context (preferences, decisions, file paths, summaries) \
             to memory/{}.md. Reply with NO_REPLY if nothing to store.",
            today
        );
        let memory_flush_prompt = format!(
            "Write any lasting notes to memory/{}.md; reply with NO_REPLY if nothing to store.",
            today
        );

        let mut flush_messages: Vec<ChatMessage> = history.to_vec();
        flush_messages.push(ChatMessage::system(&memory_flush_reminder));

        let mut silent_sink = SilentEventSink;
        tracing::debug!("Running pre-compaction memory flush");
        let _ = agent_loop::run_agent_loop(
            &self.config,
            flush_messages,
            &memory_flush_prompt,
            &self.skills,
            &mut silent_sink,
            Some(&self.session_key),
        )
        .await?;
        Ok(())
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

    /// Full clear (OpenClaw-style): summarize to memory, archive transcript, reset counts.
    /// Used by Assistant /new and `skilllite clear-session`.
    pub async fn clear_full(&mut self) -> Result<()> {
        if let Ok(history) = self.read_history() {
            if !history.is_empty() {
                let _ = self.summarize_for_memory(&history).await;
            }
        }
        self.archive_transcript()?;
        self.reset_session_counts()?;
        self.session_id = None;
        Ok(())
    }

    fn archive_transcript(&self) -> Result<()> {
        let transcripts_dir = self.data_root.join("transcripts");
        let paths = transcript::list_transcript_files(&transcripts_dir, &self.session_key)?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for path in paths {
            let archived =
                std::path::PathBuf::from(format!("{}.archived.{}", path.display(), timestamp));
            std::fs::rename(&path, &archived)?;
        }
        Ok(())
    }

    fn reset_session_counts(&self) -> Result<()> {
        let sessions_path = self.data_root.join("sessions.json");
        if let Ok(mut store) = session::SessionStore::load(&sessions_path) {
            store.reset_compaction_state(&self.session_key);
            let _ = store.save(&sessions_path);
        }
        Ok(())
    }

    /// Clear session: summarize conversation to memory, then reset (CLI /clear, transcript kept).
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
        // clear-session should still finish quickly without an API key.
        if self.config.api_key.trim().is_empty() {
            tracing::info!("Skipping memory summary on clear: OPENAI_API_KEY is empty");
            return Ok(());
        }

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

        let memory_entry = format!(
            "\n\n---\n\n## [Session cleared — {}]\n\n{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M"),
            summary
        );

        // Write to memory/YYYY-MM-DD.md (durable, searchable)
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let memory_dir = self.data_root.join("memory");
        std::fs::create_dir_all(&memory_dir)?;
        let memory_path = memory_dir.join(format!("{}.md", today));
        let final_content = if memory_path.exists() {
            format!(
                "{}\n{}",
                std::fs::read_to_string(&memory_path).unwrap_or_default(),
                memory_entry
            )
        } else {
            memory_entry.trim_start().to_string()
        };
        std::fs::write(&memory_path, &final_content)?;

        // Index for BM25 search
        let rel_path = format!("{}.md", today);
        let idx_path = executor_memory::index_path(&self.data_root, &self.session_key);
        if let Some(parent) = idx_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Ok(conn) = rusqlite::Connection::open(&idx_path) {
            let _ = executor_memory::ensure_index(&conn)
                .and_then(|_| executor_memory::index_file(&conn, &rel_path, &final_content));
        }

        tracing::info!("Session memory summary written to memory/{}", rel_path);

        // Also append compaction to transcript so read_history returns summary (CLI /clear case)
        let transcripts_dir = self.data_root.join("transcripts");
        let t_path = transcript::transcript_path_today(&transcripts_dir, &self.session_key);
        let entry = transcript::TranscriptEntry::Compaction {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            first_kept_entry_id: String::new(),
            tokens_before: 0,
            summary: Some(format!("[Session cleared — memory summary]\n{}", summary)),
        };
        let _ = transcript::append_entry(&t_path, &entry);

        Ok(())
    }
}

// ─── A9: Evolution triggers (periodic + decision-count) ─────────────────────

/// Run evolution once and emit summary. Shared by periodic and decision-count triggers.
/// workspace: project root for skill evolution (skills written to workspace/.skills/_evolved/).
async fn run_evolution_and_emit_summary(
    data_root: &PathBuf,
    workspace: &str,
    api_base: &str,
    api_key: &str,
    model: &str,
) {
    let skills_root = if workspace.is_empty() {
        None
    } else {
        let ws = std::path::Path::new(workspace);
        let sr = if ws.is_absolute() {
            ws.join(".skills")
        } else {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(workspace)
                .join(".skills")
        };
        Some(sr)
    };
    let llm = LlmClient::new(api_base, api_key);
    let adapter = evolution::EvolutionLlmAdapter { llm: &llm };
    let skills_root_ref = skills_root.as_deref();
    match skilllite_evolution::run_evolution(
        data_root,
        skills_root_ref,
        &adapter,
        api_base,
        api_key,
        model,
        false,
    )
    .await
    {
        Ok(Some(txn_id)) => {
            tracing::info!("Evolution completed: {}", txn_id);
            if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(data_root) {
                let changes = skilllite_evolution::query_changes_by_txn(&conn, &txn_id);
                for msg in &skilllite_evolution::format_evolution_changes(&changes) {
                    eprintln!("{}", msg);
                }
                let _ = skilllite_evolution::check_auto_rollback(&conn, data_root);
            }
        }
        Ok(None) => tracing::debug!("Evolution: nothing to evolve"),
        Err(e) => tracing::warn!("Evolution failed: {}", e),
    }
}

/// A9: Periodic evolution trigger — runs every N seconds, even when user is active.
pub fn spawn_periodic_evolution(
    data_root: PathBuf,
    workspace: String,
    api_base: String,
    api_key: String,
    model: String,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if skilllite_evolution::EvolutionMode::from_env().is_disabled() {
            tracing::debug!("Evolution disabled, skipping periodic trigger");
            return;
        }
        let interval = std::time::Duration::from_secs(interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            tracing::debug!("Periodic evolution trigger fired (every {}s)", interval_secs);
            run_evolution_and_emit_summary(&data_root, workspace.as_str(), &api_base, &api_key, &model).await;
        }
    })
}

/// A9: Decision-count trigger — spawn evolution once when threshold is met.
pub fn spawn_evolution_once(
    data_root: PathBuf,
    workspace: String,
    api_base: String,
    api_key: String,
    model: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if skilllite_evolution::EvolutionMode::from_env().is_disabled() {
            return;
        }
        tracing::debug!("Decision-count evolution trigger fired");
        run_evolution_and_emit_summary(&data_root, workspace.as_str(), &api_base, &api_key, &model).await;
    })
}

/// Shutdown hook: flush metrics, no LLM calls. Called before process exit.
pub fn shutdown_evolution(data_root: &std::path::Path) {
    skilllite_evolution::on_shutdown(data_root);
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
