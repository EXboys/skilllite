//! Execution sub-module: tool-call batch processing for the agent loop.
//!
//! Handles progressive disclosure, per-task call depth, update_task_plan
//! replan, failure tracking, and result processing for both simple and
//! task-planning loop paths.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use super::helpers::{
    execute_tool_call, handle_update_task_plan, inject_progressive_disclosure,
    process_result_content,
};
use super::super::extensions::{self, MemoryVectorContext};
use super::super::llm::LlmClient;
use super::super::skills::LoadedSkill;
use super::super::task_planner::TaskPlanner;
use super::super::types::*;

/// Helper to get current timestamp string
fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

/// Write tool call and result to transcript for complete traceability
/// (aligned with OpenAI Agents SDK tracing and Claude Code format)
fn write_tool_to_transcript(
    session_key: Option<&str>,
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    result: &str,
    is_error: bool,
    elapsed_ms: Option<u64>,
) {
    let session_key = match session_key {
        Some(s) => s,
        None => return,
    };

    // Get transcript path
    let data_root = match skilllite_executor::workspace_root(None) {
        Ok(p) => p,
        Err(_) => return,
    };
    let transcripts_dir = data_root.join("transcripts");
    let t_path = match skilllite_executor::transcript::transcript_path_today(&transcripts_dir, session_key) {
        p if p.parent().map(|p| skilllite_fs::create_dir_all(p).is_ok()).unwrap_or(false) => p,
        _ => return,
    };

    let now = timestamp_now();

    // Write ToolCall entry
    let tool_call_entry = skilllite_executor::transcript::TranscriptEntry::ToolCall {
        id: uuid::Uuid::new_v4().to_string(),
        parent_id: None,
        tool_call_id: tool_call_id.to_string(),
        name: name.to_string(),
        arguments: arguments.to_string(),
        timestamp: now.clone(),
    };
    let _ = skilllite_executor::transcript::append_entry(&t_path, &tool_call_entry);

    // Write ToolResult entry
    let tool_result_entry = skilllite_executor::transcript::TranscriptEntry::ToolResult {
        id: uuid::Uuid::new_v4().to_string(),
        parent_id: None,
        tool_call_id: tool_call_id.to_string(),
        name: name.to_string(),
        result: result.to_string(),
        is_error,
        elapsed_ms,
        timestamp: now,
    };
    let _ = skilllite_executor::transcript::append_entry(&t_path, &tool_result_entry);
}

// ── Shared state ─────────────────────────────────────────────────────────────

/// Mutable counters accumulated across all loop iterations.
pub(super) struct ExecutionState {
    pub total_tool_calls: usize,
    pub failed_tool_calls: usize,
    pub consecutive_failures: usize,
    /// Calls since the last per-task depth reset.
    pub tool_calls_current_task: usize,
    pub replan_count: usize,
    pub tools_detail: Vec<ToolExecDetail>,
    pub context_overflow_retries: usize,
    pub iterations: usize,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            total_tool_calls: 0,
            failed_tool_calls: 0,
            consecutive_failures: 0,
            tool_calls_current_task: 0,
            replan_count: 0,
            tools_detail: Vec::new(),
            context_overflow_retries: 0,
            iterations: 0,
        }
    }
}

/// What the caller should do after executing a tool batch.
pub(super) struct ToolBatchOutcome {
    /// Progressive disclosure injected — caller should re-prompt (continue).
    pub disclosure_injected: bool,
    /// Consecutive-failure limit reached — caller should stop.
    pub failure_limit_reached: bool,
    /// Per-task call depth reached — caller should inject depth-limit message.
    pub depth_limit_reached: bool,
}

// ── Auto-mark on skill success ─────────────────────────────────────────────────

/// If the tool result contains `"success": true` and the tool matches the current
/// task's tool_hint, mark that task completed. Reduces redundant retries when the
/// LLM doesn't call update_task_plan.
fn try_auto_mark_task_on_success(
    planner: &mut TaskPlanner,
    tool_name: &str,
    result_content: &str,
    event_sink: &mut dyn EventSink,
) {
    let task_id = match planner.current_task() {
        Some(t) => {
            let h = t.tool_hint.as_deref().map(|x| x.replace('-', "_").to_lowercase());
            let tn = tool_name.replace('-', "_").to_lowercase();
            if h.as_deref() == Some(tn.as_str()) { t.id } else { return }
        }
        None => return,
    };
    // Check for success in JSON (skills typically return {"success": true, ...})
    if !result_content.contains("\"success\"") || !result_content.contains("true") {
        return;
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(result_content) {
        if v.get("success").and_then(|s| s.as_bool()) != Some(true) {
            return;
        }
    } else {
        return;
    }
    planner.mark_completed(task_id);
    event_sink.on_task_progress(task_id, true);
    tracing::info!("Auto-marked task {} completed (skill {} succeeded)", task_id, tool_name);
}

// ── Planning-mode batch ───────────────────────────────────────────────────────

/// Execute a batch of tool calls in **planning mode** (supports `update_task_plan`).
///
/// Updates `state` in place. Returns a `ToolBatchOutcome` the caller uses to
/// decide whether to `continue`, inject a depth-limit message, or stop.
#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_tool_batch_planning(
    tool_calls: &[ToolCall],
    registry: &extensions::ExtensionRegistry<'_>,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
    embed_ctx: Option<&MemoryVectorContext<'_>>,
    client: &LlmClient,
    model: &str,
    planner: &mut TaskPlanner,
    skills: &[LoadedSkill],
    messages: &mut Vec<ChatMessage>,
    documented_skills: &mut HashSet<String>,
    state: &mut ExecutionState,
    max_tool_calls_per_task: usize,
    max_consecutive_failures: Option<usize>,
    session_key: Option<&str>,
) -> ToolBatchOutcome {
    if inject_progressive_disclosure(tool_calls, skills, documented_skills, messages) {
        return ToolBatchOutcome { disclosure_injected: true, failure_limit_reached: false, depth_limit_reached: false };
    }

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);

        let is_replan = tool_name.as_str() == "update_task_plan";
        let start_time = Instant::now();
        let mut result = if is_replan {
            state.replan_count += 1;
            handle_update_task_plan(arguments, planner, event_sink)
        } else {
            execute_tool_call(registry, tool_name, arguments, workspace, event_sink, embed_ctx).await
        };
        result.tool_call_id = tc.id.clone();
        result.content = process_result_content(client, model, tool_name, &result.content).await;

        if result.is_error {
            state.failed_tool_calls += 1;
            state.consecutive_failures += 1;
            result.content.push_str("\n\nTip: Consider update_task_plan if the approach needs to change.");
        } else {
            state.consecutive_failures = 0;
            // Auto-mark task when skill succeeds and matches current task's tool_hint
            if !is_replan {
                try_auto_mark_task_on_success(planner, tool_name, &result.content, event_sink);
            }
        }
        if !is_replan {
            state.tools_detail.push(ToolExecDetail { tool: tool_name.clone(), success: !result.is_error });
        }

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        // Write to transcript for complete traceability
        write_tool_to_transcript(
            session_key,
            &tc.id,
            tool_name,
            arguments,
            &result.content,
            result.is_error,
            Some(elapsed_ms),
        );

        event_sink.on_tool_result(tool_name, &result.content, result.is_error);
        messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));
        state.total_tool_calls += 1;
        state.tool_calls_current_task += 1;
    }

    let failure_limit_reached = max_consecutive_failures
        .map_or(false, |limit| state.consecutive_failures >= limit);
    let depth_limit_reached = state.tool_calls_current_task >= max_tool_calls_per_task;

    ToolBatchOutcome { disclosure_injected: false, failure_limit_reached, depth_limit_reached }
}

// ── Simple-mode batch ─────────────────────────────────────────────────────────

/// Execute a batch of tool calls in **simple mode** (no planning, no replan).
#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_tool_batch_simple(
    tool_calls: &[ToolCall],
    registry: &extensions::ExtensionRegistry<'_>,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
    embed_ctx: Option<&MemoryVectorContext<'_>>,
    client: &LlmClient,
    model: &str,
    skills: &[LoadedSkill],
    messages: &mut Vec<ChatMessage>,
    documented_skills: &mut HashSet<String>,
    state: &mut ExecutionState,
    max_consecutive_failures: Option<usize>,
    session_key: Option<&str>,
) -> ToolBatchOutcome {
    if inject_progressive_disclosure(tool_calls, skills, documented_skills, messages) {
        return ToolBatchOutcome { disclosure_injected: true, failure_limit_reached: false, depth_limit_reached: false };
    }

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);

        let start_time = Instant::now();
        let mut result = execute_tool_call(registry, tool_name, arguments, workspace, event_sink, embed_ctx).await;
        result.tool_call_id = tc.id.clone();
        result.content = process_result_content(client, model, tool_name, &result.content).await;

        if result.is_error {
            state.failed_tool_calls += 1;
            state.consecutive_failures += 1;
        } else {
            state.consecutive_failures = 0;
        }
        state.tools_detail.push(ToolExecDetail { tool: tool_name.clone(), success: !result.is_error });

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        // Write to transcript for complete traceability
        write_tool_to_transcript(
            session_key,
            &tc.id,
            tool_name,
            arguments,
            &result.content,
            result.is_error,
            Some(elapsed_ms),
        );

        event_sink.on_tool_result(tool_name, &result.content, result.is_error);
        messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));
        state.total_tool_calls += 1;
    }

    let failure_limit_reached = max_consecutive_failures
        .map_or(false, |limit| state.consecutive_failures >= limit);
    ToolBatchOutcome { disclosure_injected: false, failure_limit_reached, depth_limit_reached: false }
}

