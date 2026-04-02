//! Chat session: persistent conversation with transcript and memory.
//!
//! Ported from Python `ChatSession`. Directly calls executor module
//! (same process, no IPC). Handles transcript persistence, auto-compaction,
//! and memory integration.

use crate::Result;
use anyhow::Context;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use skilllite_executor::{memory as executor_memory, session, transcript};

use super::agent_loop;
use super::evolution;
use super::extensions;
use super::llm::LlmClient;
use super::long_text;
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
    transcript_cache: TranscriptCache,
}

#[derive(Default)]
struct TranscriptCache {
    files: HashMap<PathBuf, CachedTranscriptFile>,
}

#[derive(Default)]
struct CachedTranscriptFile {
    offset: u64,
    entries: Vec<transcript::TranscriptEntry>,
}

impl ChatSession {
    /// Full constructor for long-lived chat. Evolution runs are **not** spawned here;
    /// desktop **Life Pulse** (or manual `skilllite evolution run`) drives A9 triggers.
    pub fn new(config: AgentConfig, session_key: &str, skills: Vec<LoadedSkill>) -> Self {
        Self::new_inner(config, session_key, skills)
    }

    /// For one-off clear-session: no Tokio spawn. Avoids "no reactor running" when run from sync CLI.
    pub fn new_for_clear(config: AgentConfig, session_key: &str, skills: Vec<LoadedSkill>) -> Self {
        Self::new_inner(config, session_key, skills)
    }

    fn new_inner(config: AgentConfig, session_key: &str, skills: Vec<LoadedSkill>) -> Self {
        let data_root = skilllite_executor::chat_root();
        skilllite_evolution::seed::ensure_seed_data(&data_root);
        Self {
            config,
            session_key: session_key.to_string(),
            session_id: None,
            data_root,
            skills,
            transcript_cache: TranscriptCache::default(),
        }
    }

    /// Path to today's append-only transcript file for this session (same file as `ensure_session`).
    /// Used by agent RPC to persist desktop-only UI rows (e.g. confirmation/clarification) without
    /// affecting LLM history (`read_history` ignores `custom_message` entries).
    pub fn transcript_append_path(&self) -> PathBuf {
        let transcripts_dir = self.data_root.join("transcripts");
        transcript::transcript_path_today(&transcripts_dir, &self.session_key)
    }

    /// Ensure session and transcript exist, return session_id.
    fn ensure_session(&mut self) -> Result<String> {
        if let Some(ref id) = self.session_id {
            return Ok(id.clone());
        }

        // Ensure data_root directory exists
        if !self.data_root.exists() {
            skilllite_fs::create_dir_all(&self.data_root)?;
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
    fn read_history(&mut self) -> Result<Vec<ChatMessage>> {
        let entries = self.read_history_entries_incremental()?;
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

    fn read_history_entries_incremental(&mut self) -> Result<Vec<&transcript::TranscriptEntry>> {
        let transcripts_dir = self.data_root.join("transcripts");
        let paths = transcript::list_transcript_files(&transcripts_dir, &self.session_key)?;
        self.transcript_cache
            .files
            .retain(|path, _| paths.contains(path));

        for path in &paths {
            let len = std::fs::metadata(path)
                .with_context(|| format!("Failed to stat transcript: {}", path.display()))?
                .len();

            let cache = self.transcript_cache.files.entry(path.clone()).or_default();

            // File rotation/truncation: reset offset and replay from start.
            if len < cache.offset {
                cache.offset = 0;
                cache.entries.clear();
            }

            if len > cache.offset {
                let (new_entries, next_offset) = read_entries_from_offset(path, cache.offset)?;
                cache
                    .entries
                    .extend(new_entries.into_iter().filter(is_history_relevant_entry));
                cache.offset = next_offset;
            }
        }
        prune_cache_before_last_compaction(&mut self.transcript_cache, &paths);
        apply_message_window_to_cache(
            &mut self.transcript_cache,
            &paths,
            history_window_messages_limit(),
        );

        let mut entries = Vec::new();
        for path in &paths {
            if let Some(cache) = self.transcript_cache.files.get(path) {
                entries.extend(cache.entries.iter());
            }
        }
        Ok(entries)
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
                            tracing::debug!(
                                "Early memory flush completed (threshold={})",
                                flush_threshold
                            );
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

        // ── Guard #1: truncate oversized user messages already in history ──────
        // Handles old transcripts written before the compression fix.
        // Sync simple truncation only — no LLM call here, too expensive per-turn.
        {
            let max_chars = get_user_input_max_chars();
            for msg in history.iter_mut() {
                if msg.role == "user" {
                    if let Some(ref content) = msg.content {
                        if content.len() > max_chars {
                            tracing::debug!(
                                len = content.len(),
                                max_chars,
                                "Truncating oversized historical user message"
                            );
                            msg.content = Some(long_text::truncate_content(content, max_chars));
                        }
                    }
                }
            }
        }

        // Build memory context (if enabled) — inject relevant memories as system context
        // Uses original user_message for accurate intent-based vector search.
        if self.config.enable_memory {
            let workspace = std::path::Path::new(&self.config.workspace);
            if let Some(mem_ctx) =
                extensions::build_memory_context(workspace, "default", user_message)
            {
                history.push(ChatMessage::system(&mem_ctx));
            }
        }

        // ── Guard #2: compress current user message if oversized ─────────────
        // Processed BEFORE transcript write so the stored version is already
        // compressed — read_history on next turn gets the compressed version directly.
        let client = LlmClient::new(&self.config.api_base, &self.config.api_key)?;
        let effective_user_message =
            long_text::maybe_process_user_input(&client, &self.config.model, user_message).await;

        // Append (compressed) user message to transcript
        self.append_message("user", &effective_user_message)?;

        event_sink.on_turn_start();

        // Run the agent loop — receives the already-compressed message.
        // Note: update_previous_feedback and build_memory_context above intentionally
        // use the original user_message for accurate intent matching.
        let result = agent_loop::run_agent_loop(
            &self.config,
            history,
            &effective_user_message,
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

        // Tool call / result lines are appended during execution (`execution::append_*_to_transcript`)
        // so order matches the UI (tool_call → optional confirmation custom_message → tool_result).

        // Append assistant response to transcript
        self.append_message("assistant", &result.response)?;

        // EVO-1: Record execution decision (async-safe, <1ms with WAL).
        // Only record meaningful turns (at least 1 tool call).
        if result.feedback.total_tools >= 1 {
            // EVO-1: monitoring only (decisions + metrics). A9 execution is desktop Life Pulse or CLI.
            self.record_decision(&result.feedback);
        }

        Ok(result)
    }

    /// Graceful shutdown: flush evolution metrics (no LLM).
    pub fn shutdown(&mut self) {
        shutdown_evolution(&self.data_root);
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
        Ok(transcript::append_entry(&t_path, &entry)?)
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
    async fn compact_history(&mut self, history: Vec<ChatMessage>) -> Result<Vec<ChatMessage>> {
        let threshold = get_compaction_threshold();
        if history.len() < threshold {
            return Ok(history);
        }

        // Pre-compaction memory flush (OpenClaw-style): give model a chance to save to memory
        // before we summarize away the conversation. Runs once per compaction cycle.
        if self.config.enable_memory && get_memory_flush_enabled() {
            let sessions_path = self.data_root.join("sessions.json");
            if let Ok(store) = session::SessionStore::load(&sessions_path) {
                if let Some(entry) = store.get(&self.session_key) {
                    let next_compaction_count = entry.compaction_count + 1;
                    let need_flush =
                        entry.memory_flush_compaction_count != Some(next_compaction_count);
                    if need_flush {
                        if let Err(e) = self.run_memory_flush_turn(&history).await {
                            tracing::warn!(
                                "Memory flush failed (continuing with compaction): {}",
                                e
                            );
                        } else if let Ok(mut store) = session::SessionStore::load(&sessions_path) {
                            if let Some(session_entry) = store.sessions.get_mut(&self.session_key) {
                                session_entry.memory_flush_compaction_count =
                                    Some(next_compaction_count);
                                session_entry.memory_flush_at =
                                    Some(chrono::Utc::now().to_rfc3339());
                                let _ = store.save(&sessions_path);
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
        let client = LlmClient::new(&self.config.api_base, &self.config.api_key)?;
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
        self.transcript_cache = TranscriptCache::default();
        Ok(())
    }

    fn archive_transcript(&mut self) -> Result<()> {
        let transcripts_dir = self.data_root.join("transcripts");
        let paths = transcript::list_transcript_files(&transcripts_dir, &self.session_key)?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for path in paths {
            let archived =
                std::path::PathBuf::from(format!("{}.archived.{}", path.display(), timestamp));
            skilllite_fs::rename(&path, &archived)?;
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
        self.transcript_cache = TranscriptCache::default();
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

        let client = LlmClient::new(&self.config.api_base, &self.config.api_key)?;

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
        skilllite_fs::create_dir_all(&memory_dir)?;
        let memory_path = memory_dir.join(format!("{}.md", today));
        let final_content = if memory_path.exists() {
            format!(
                "{}\n{}",
                skilllite_fs::read_file(&memory_path).unwrap_or_default(),
                memory_entry
            )
        } else {
            memory_entry.trim_start().to_string()
        };
        skilllite_fs::write_file(&memory_path, &final_content)?;

        // Index for BM25 search
        let rel_path = format!("{}.md", today);
        let idx_path = executor_memory::index_path(&self.data_root, &self.session_key);
        if let Some(parent) = idx_path.parent() {
            skilllite_fs::create_dir_all(parent)?;
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

fn read_entries_from_offset(
    transcript_path: &Path,
    offset: u64,
) -> Result<(Vec<transcript::TranscriptEntry>, u64)> {
    let file = File::open(transcript_path)
        .with_context(|| format!("Failed to open transcript: {}", transcript_path.display()))?;
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(offset))
        .with_context(|| format!("Failed to seek transcript: {}", transcript_path.display()))?;

    let mut entries = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .with_context(|| format!("Failed to read transcript: {}", transcript_path.display()))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let entry: transcript::TranscriptEntry =
            serde_json::from_str(trimmed).with_context(|| {
                format!(
                    "Failed to parse transcript line: {}",
                    transcript_path.display()
                )
            })?;
        entries.push(entry);
    }
    let next_offset = reader.stream_position().with_context(|| {
        format!(
            "Failed to read stream position: {}",
            transcript_path.display()
        )
    })?;
    Ok((entries, next_offset))
}

fn is_history_relevant_entry(entry: &transcript::TranscriptEntry) -> bool {
    matches!(
        entry,
        transcript::TranscriptEntry::Message { .. }
            | transcript::TranscriptEntry::Compaction { .. }
    )
}

fn history_window_messages_limit() -> usize {
    std::env::var("SKILLLITE_HISTORY_WINDOW_MESSAGES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(200)
}

fn prune_cache_before_last_compaction(cache: &mut TranscriptCache, paths: &[PathBuf]) {
    let mut compaction_position: Option<(usize, usize)> = None;
    for (path_idx, path) in paths.iter().enumerate() {
        if let Some(file) = cache.files.get(path) {
            if let Some(entry_idx) = file
                .entries
                .iter()
                .rposition(|e| matches!(e, transcript::TranscriptEntry::Compaction { .. }))
            {
                compaction_position = Some((path_idx, entry_idx));
            }
        }
    }

    let Some((compaction_file_idx, compaction_entry_idx)) = compaction_position else {
        return;
    };

    for old_path in &paths[..compaction_file_idx] {
        if let Some(file) = cache.files.get_mut(old_path) {
            file.entries.clear();
        }
    }

    if let Some(file) = cache.files.get_mut(&paths[compaction_file_idx]) {
        if compaction_entry_idx > 0 {
            file.entries.drain(0..compaction_entry_idx);
        }
    }
}

fn apply_message_window_to_cache(cache: &mut TranscriptCache, paths: &[PathBuf], limit: usize) {
    if limit == 0 {
        return;
    }

    let mut total_messages = paths
        .iter()
        .filter_map(|path| cache.files.get(path))
        .flat_map(|file| file.entries.iter())
        .filter(|entry| matches!(entry, transcript::TranscriptEntry::Message { .. }))
        .count();

    if total_messages <= limit {
        return;
    }

    let mut remaining_to_drop = total_messages - limit;
    for path in paths {
        if remaining_to_drop == 0 {
            break;
        }
        let Some(file) = cache.files.get_mut(path) else {
            continue;
        };

        let has_head_compaction = matches!(
            file.entries.first(),
            Some(transcript::TranscriptEntry::Compaction { .. })
        );
        let mut at_head = true;
        file.entries.retain(|entry| {
            if remaining_to_drop == 0 {
                at_head = false;
                return true;
            }
            // Keep a compaction marker at file head so read_history semantics stay intact.
            if at_head && has_head_compaction {
                at_head = false;
                return true;
            }
            at_head = false;
            if matches!(entry, transcript::TranscriptEntry::Message { .. }) && remaining_to_drop > 0
            {
                remaining_to_drop -= 1;
                false
            } else {
                true
            }
        });
    }
    total_messages = paths
        .iter()
        .filter_map(|path| cache.files.get(path))
        .flat_map(|file| file.entries.iter())
        .filter(|entry| matches!(entry, transcript::TranscriptEntry::Message { .. }))
        .count();
    debug_assert!(total_messages <= limit || remaining_to_drop == 0);
}

/// Shutdown hook: flush metrics, no LLM calls. Called before process exit.
pub fn shutdown_evolution(data_root: &std::path::Path) {
    skilllite_evolution::on_shutdown(data_root);
}

/// Convert a transcript entry to a ChatMessage.
fn transcript_entry_to_message(entry: &transcript::TranscriptEntry) -> Option<ChatMessage> {
    match entry {
        transcript::TranscriptEntry::Message { role, content, .. } => Some(ChatMessage {
            role: role.clone(),
            content: content.clone(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }),
        transcript::TranscriptEntry::Compaction { summary, .. } => summary
            .as_ref()
            .map(|s| ChatMessage::system(&format!("[Previous conversation summary]\n{}", s))),
        _ => None,
    }
}

#[cfg(test)]
mod history_window_tests {
    use super::*;

    fn msg(content: &str) -> transcript::TranscriptEntry {
        transcript::TranscriptEntry::Message {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
        }
    }

    fn compaction() -> transcript::TranscriptEntry {
        transcript::TranscriptEntry::Compaction {
            id: uuid::Uuid::new_v4().to_string(),
            parent_id: None,
            first_kept_entry_id: String::new(),
            tokens_before: 0,
            summary: Some("summary".to_string()),
        }
    }

    #[test]
    fn prune_before_last_compaction_removes_old_files() {
        let p1 = PathBuf::from("day1");
        let p2 = PathBuf::from("day2");
        let p3 = PathBuf::from("day3");
        let mut cache = TranscriptCache::default();
        cache.files.insert(
            p1.clone(),
            CachedTranscriptFile {
                offset: 0,
                entries: vec![msg("a"), msg("b")],
            },
        );
        cache.files.insert(
            p2.clone(),
            CachedTranscriptFile {
                offset: 0,
                entries: vec![msg("c"), compaction(), msg("d")],
            },
        );
        cache.files.insert(
            p3.clone(),
            CachedTranscriptFile {
                offset: 0,
                entries: vec![msg("e")],
            },
        );

        prune_cache_before_last_compaction(&mut cache, &[p1.clone(), p2.clone(), p3.clone()]);

        assert!(cache.files.get(&p1).unwrap().entries.is_empty());
        assert!(matches!(
            cache.files.get(&p2).unwrap().entries.first(),
            Some(transcript::TranscriptEntry::Compaction { .. })
        ));
    }

    #[test]
    fn apply_window_keeps_recent_messages() {
        let p = PathBuf::from("day");
        let mut cache = TranscriptCache::default();
        cache.files.insert(
            p.clone(),
            CachedTranscriptFile {
                offset: 0,
                entries: vec![compaction(), msg("1"), msg("2"), msg("3"), msg("4")],
            },
        );

        apply_message_window_to_cache(&mut cache, std::slice::from_ref(&p), 2);
        let entries = &cache.files.get(&p).unwrap().entries;
        let kept_messages = entries
            .iter()
            .filter(|e| matches!(e, transcript::TranscriptEntry::Message { .. }))
            .count();
        assert_eq!(kept_messages, 2);
        assert!(matches!(
            entries.first(),
            Some(transcript::TranscriptEntry::Compaction { .. })
        ));
    }
}
