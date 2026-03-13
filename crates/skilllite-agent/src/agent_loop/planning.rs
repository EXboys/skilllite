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

    // Build conversation context for "继续" detection.
    // Callers can set config.skip_history_for_planning=true to exclude transcript history
    // from the planning prompt (e.g. when each task is self-contained and history would
    // corrupt planning with unrelated tasks from previous turns).
    let conversation_context: Option<String> = if config.skip_history_for_planning {
        tracing::debug!("skip_history_for_planning=true: excluding transcript from planning prompt");
        None
    } else {
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
            config.context_append.as_deref(),
        )
    } else {
        let mut p = planner.build_task_system_prompt(skills, effective_boundaries.as_ref());
        if let Some(s) = &soul {
            p = format!("{}\n\n{}", s.to_system_prompt_block(), p);
        }
        if let Some(ref ctx) = config.context_append {
            if !ctx.is_empty() {
                p.push_str(&format!("\n\n{}", ctx.trim()));
            }
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

    // Assemble initial messages.
    // user_message is already compressed by chat_session before reaching here.
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
///
/// `tools_already_called`: deduplicated list of tool names successfully called
/// so far in this session. Injected so the LLM can avoid redundant calls
/// (e.g. not re-calling preview_server when the server is already running).
pub(super) fn build_task_focus_message(
    planner: &TaskPlanner,
    tools_already_called: &[String],
) -> Option<String> {
    let current = planner.current_task()?;
    let tool_hint = current.tool_hint.as_deref().unwrap_or("");
    let pending_tasks = planner.task_list.iter().filter(|t| !t.completed).count();
    let preferred_tools = TaskPlanner::preferred_tool_names_for_hint(tool_hint).join(",");
    let already_called = if tools_already_called.is_empty() {
        "none".to_string()
    } else {
        tools_already_called.join(",")
    };

    Some(format!(
        "[internal_task_focus]\n\
current_task_id={}\n\
pending_tasks={}\n\
tool_hint={}\n\
already_called_this_session={}\n\
final_summary_allowed=false\n\
replan_allowed=true\n\
preferred_tools={}\n\
do_not_quote_or_repeat_this_block=true\n\
[/internal_task_focus]",
        current.id,
        pending_tasks,
        if tool_hint.is_empty() { "none" } else { tool_hint },
        already_called,
        if preferred_tools.is_empty() { "none".to_string() } else { preferred_tools }
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_task_focus_message_uses_internal_control_block() {
        let mut planner = TaskPlanner::new(None, None);
        planner.task_list = vec![
            Task {
                id: 1,
                description: "Write the page".to_string(),
                tool_hint: Some("file_write".to_string()),
                completed: false,
            },
            Task {
                id: 2,
                description: "Preview the page".to_string(),
                tool_hint: Some("preview".to_string()),
                completed: false,
            },
        ];

        let msg = build_task_focus_message(&planner, &[]).unwrap();
        assert!(msg.contains("[internal_task_focus]"));
        assert!(msg.contains("current_task_id=1"));
        assert!(msg.contains("pending_tasks=2"));
        assert!(msg.contains("tool_hint=file_write"));
        assert!(msg.contains("already_called_this_session=none"));
        assert!(msg.contains("final_summary_allowed=false"));
        assert!(msg.contains("preferred_tools=write_file,write_output"));
        assert!(!msg.contains("Task progress update"));
        assert!(!msg.contains("\"id\": 1"));
        assert!(!msg.contains("Preferred tools:"));

        // With tools already called
        let tools = vec!["write_file".to_string(), "preview_server".to_string()];
        let msg2 = build_task_focus_message(&planner, &tools).unwrap();
        assert!(msg2.contains("already_called_this_session=write_file,preview_server"));
    }
}

