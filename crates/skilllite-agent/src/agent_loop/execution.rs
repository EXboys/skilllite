//! Execution sub-module: tool-call batch processing for the agent loop.
//!
//! Handles progressive disclosure, per-task call depth, update_task_plan
//! replan, failure tracking, and result processing for both simple and
//! task-planning loop paths.

use std::collections::HashSet;
use std::path::Path;

use super::helpers::{
    execute_tool_call, handle_update_task_plan, inject_progressive_disclosure,
    process_result_content,
};
use super::super::extensions::{self, MemoryVectorContext};
use super::super::llm::LlmClient;
use super::super::skills::LoadedSkill;
use super::super::task_planner::TaskPlanner;
use super::super::types::*;

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
) -> ToolBatchOutcome {
    if inject_progressive_disclosure(tool_calls, skills, documented_skills, messages) {
        return ToolBatchOutcome { disclosure_injected: true, failure_limit_reached: false, depth_limit_reached: false };
    }

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);

        let is_replan = tool_name.as_str() == "update_task_plan";
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
        }
        if !is_replan {
            state.tools_detail.push(ToolExecDetail { tool: tool_name.clone(), success: !result.is_error });
        }

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
) -> ToolBatchOutcome {
    if inject_progressive_disclosure(tool_calls, skills, documented_skills, messages) {
        return ToolBatchOutcome { disclosure_injected: true, failure_limit_reached: false, depth_limit_reached: false };
    }

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);

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

        event_sink.on_tool_result(tool_name, &result.content, result.is_error);
        messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));
        state.total_tool_calls += 1;
    }

    let failure_limit_reached = max_consecutive_failures
        .map_or(false, |limit| state.consecutive_failures >= limit);
    ToolBatchOutcome { disclosure_injected: false, failure_limit_reached, depth_limit_reached: false }
}

