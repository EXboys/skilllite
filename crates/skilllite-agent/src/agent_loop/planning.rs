//! Planning sub-module: pre-loop setup and LLM-driven task generation.
//!
//! Extracts everything that happens *before* the execution loop in
//! `run_with_task_planning`: soul loading, goal-boundary extraction,
//! task-list generation, system-prompt building, and checkpoint saving.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::helpers::extract_goal_boundaries_hybrid;
use super::super::goal_boundaries;
use super::super::llm::LlmClient;
use super::super::prompt;
use super::super::skills::LoadedSkill;
use super::super::soul::Soul;
use super::super::task_planner::TaskPlanner;
use super::super::types::*;

/// Output of the planning phase, consumed by the execution loop.
pub(super) struct PlanningResult {
    pub planner: TaskPlanner,
    pub messages: Vec<ChatMessage>,
    pub chat_root: PathBuf,
}

/// Run the full planning phase for `run_with_task_planning`.
///
/// Covers: soul loading, goal-boundary extraction (regex + optional LLM),
/// task-list generation via LLM, system-prompt building, initial message
/// construction, and initial run-mode checkpoint.
pub(super) async fn run_planning_phase(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
    client: &LlmClient,
    workspace: &Path,
) -> Result<PlanningResult> {
    let chat_root = skilllite_executor::workspace_root(None)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".skilllite")
        })
        .join("chat");

    let mut planner = TaskPlanner::new(Some(workspace), Some(&chat_root));

    // Build conversation context for "继续" detection
    let conversation_context: Option<String> = {
        let ctx: Vec<String> = initial_messages
            .iter()
            .filter_map(|m| m.content.as_ref().map(|c| format!("[{}] {}", m.role, c)))
            .collect();
        if ctx.is_empty() { None } else { Some(ctx.join("\n")) }
    };

    // A8: Load SOUL before planning so scope rules reach the planning prompt
    let soul = Soul::auto_load(config.soul_path.as_deref(), &config.workspace);

    // A5: Goal boundaries — hybrid (regex + optional LLM) in run mode
    let effective_boundaries = if session_key == Some("run") {
        match extract_goal_boundaries_hybrid(client, &config.model, user_message).await {
            Ok(gb) => Some(gb),
            Err(e) => {
                tracing::warn!("Goal boundaries extraction failed: {}, using regex only", e);
                Some(goal_boundaries::extract_goal_boundaries(user_message))
            }
        }
    } else {
        config.goal_boundaries.clone()
    };

    // Generate task list via LLM
    let _tasks = planner
        .generate_task_list(
            client,
            &config.model,
            user_message,
            skills,
            conversation_context.as_deref(),
            effective_boundaries.as_ref(),
            soul.as_ref(),
        )
        .await?;

    event_sink.on_task_plan(&planner.task_list);

    // Build system prompt
    let system_prompt = if planner.is_empty() {
        prompt::build_system_prompt(
            config.system_prompt.as_deref(),
            skills,
            &config.workspace,
            session_key,
            config.enable_memory,
            Some(&chat_root),
            soul.as_ref(),
        )
    } else {
        let mut p = planner.build_task_system_prompt(skills, effective_boundaries.as_ref());
        if let Some(s) = &soul {
            p = format!("{}\n\n{}", s.to_system_prompt_block(), p);
        }
        if let Some(sk) = session_key {
            p.push_str(&format!(
                "\n\nCurrent session: {} — use session_key '{}' for chat_history and chat_plan.\n\
                 /compact compresses conversation; result appears as [compaction] in chat_history. \
                 When user asks about 最新的/compact or /compact效果, read chat_history with session_key '{}'.",
                sk, sk, sk
            ));
        }
        p
    };

    // Assemble initial messages
    let mut messages = Vec::new();
    messages.push(ChatMessage::system(&system_prompt));
    messages.extend(initial_messages);
    messages.push(ChatMessage::user(user_message));

    // A13: Save initial checkpoint for --resume
    maybe_save_checkpoint(session_key, user_message, config, &planner, &messages, &chat_root);

    let _ = (soul, effective_boundaries); // used locally above; not passed to caller
    Ok(PlanningResult { planner, messages, chat_root })
}

/// Build the per-iteration task-focus message injected after tool execution.
/// Returns `None` when there is no pending task.
pub(super) fn build_task_focus_message(planner: &TaskPlanner) -> Option<String> {
    let current = planner.current_task()?;
    let task_list_json = serde_json::to_string_pretty(&planner.task_list)
        .unwrap_or_else(|_| "[]".to_string());
    let tool_hint = current.tool_hint.as_deref().unwrap_or("");
    let msg = if tool_hint == "file_operation" {
        format!(
            "Task progress update:\n{}\n\n\
             Current task to execute: Task {} - {}\n\n\
             ⚡ Use `write_output` or `preview_server` NOW. ⛔ Do NOT call any skill tools.",
            task_list_json, current.id, current.description
        )
    } else if !tool_hint.is_empty() && tool_hint != "analysis" {
        format!(
            "Task progress update:\n{}\n\n\
             Current task to execute: Task {} - {}\n\n\
             ⚡ Call `{}` DIRECTLY. Do NOT explore files first.",
            task_list_json, current.id, current.description, tool_hint
        )
    } else {
        format!(
            "Task progress update:\n{}\n\n\
             Current task to execute: Task {} - {}\n\n\
             Please continue to focus on completing the current task.",
            task_list_json, current.id, current.description
        )
    };
    Some(msg)
}

/// Save a run-mode checkpoint (A13). No-op for non-run sessions.
pub(super) fn maybe_save_checkpoint(
    session_key: Option<&str>,
    user_message: &str,
    config: &AgentConfig,
    planner: &TaskPlanner,
    messages: &[ChatMessage],
    chat_root: &Path,
) {
    if session_key != Some("run") { return; }
    let cp = crate::run_checkpoint::RunCheckpoint::new(
        user_message.to_string(),
        config.workspace.clone(),
        planner.task_list.clone(),
        messages.to_vec(),
    );
    if let Err(e) = crate::run_checkpoint::save_checkpoint(chat_root, &cp) {
        tracing::debug!("Checkpoint save failed: {}", e);
    }
}

