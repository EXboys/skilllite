//! Reflection sub-module: hallucination detection, completion checking, auto-nudge.
//!
//! Handles every "no tool calls returned" scenario in the agent loop,
//! deciding whether to nudge the LLM, accept completion claims, or stop.

use super::super::task_planner::TaskPlanner;
use super::super::types::*;

// ── Outcome ───────────────────────────────────────────────────────────────────

/// What the caller should do after the reflection phase.
pub(super) enum ReflectionOutcome {
    /// Keep looping — no nudge needed (made progress or empty first-pass).
    Continue,
    /// Inject this message as a user turn, then keep looping.
    Nudge(String),
    /// Stop the loop.
    Break,
    /// All tasks are done — stop (planning mode only).
    AllDone,
}

// ── Simple-mode reflection ────────────────────────────────────────────────────

/// Reflect on a no-tool response in **simple mode**.
///
/// Handles the anti-hallucination nudge on iteration 1 and the normal
/// "too many no-tool retries" termination condition.
pub(super) fn reflect_simple(
    assistant_content: &Option<String>,
    all_tools_len: usize,
    iterations: usize,
    no_tool_retries: &mut usize,
    max_no_tool_retries: usize,
    event_sink: &mut dyn EventSink,
    messages: &mut Vec<ChatMessage>,
) -> ReflectionOutcome {
    // Anti-hallucination nudge: first iteration, tools available, never nudged
    if iterations == 1 && all_tools_len > 0 && *no_tool_retries == 0 {
        tracing::info!(
            "First iteration produced no tool calls with {} tools available, nudging",
            all_tools_len
        );
        // Remove the text-only assistant message so it doesn't pollute history
        if messages.last().map_or(false, |m| m.role == "assistant") {
            messages.pop();
        }
        *no_tool_retries += 1;
        return ReflectionOutcome::Nudge(
            "You responded with text but did not call any tools. \
             If the task requires action (browsing, file I/O, computation, etc.), \
             you MUST call the appropriate tool functions. Do not describe what you \
             would do — actually do it by invoking the tools."
                .to_string(),
        );
    }

    // Emit final text before deciding
    if let Some(ref content) = assistant_content {
        event_sink.on_text(content);
    }

    *no_tool_retries += 1;
    if *no_tool_retries >= max_no_tool_retries || assistant_content.is_some() {
        ReflectionOutcome::Break
    } else {
        ReflectionOutcome::Continue
    }
}

// ── Planning-mode reflection ──────────────────────────────────────────────────

/// Reflect on a no-tool response in **planning mode**.
///
/// Two paths:
/// - `suppress_stream = true`: hallucination-guard path (streaming was suppressed).
/// - `suppress_stream = false`: normal post-work completion-check path.
#[allow(clippy::too_many_arguments)]
pub(super) fn reflect_planning(
    assistant_content: &Option<String>,
    suppress_stream: bool,
    planner: &mut TaskPlanner,
    consecutive_no_tool: &mut usize,
    max_no_tool_retries: usize,
    tool_calls_current_task: usize,
    total_tool_calls: usize,
    event_sink: &mut dyn EventSink,
    messages: &mut Vec<ChatMessage>,
) -> ReflectionOutcome {
    if suppress_stream {
        // Pop the hallucinated assistant message (user never saw it)
        if messages.last().map_or(false, |m| m.role == "assistant") {
            messages.pop();
        }
        tracing::info!(
            "Anti-hallucination: silently rejected text-only response \
             (no tools executed yet, plan requires tool tasks)"
        );
        *consecutive_no_tool += 1;
        if *consecutive_no_tool >= max_no_tool_retries {
            tracing::warn!(
                "LLM failed to start execution after {} attempts, stopping",
                max_no_tool_retries
            );
            return ReflectionOutcome::Break;
        }
        // Strong nudge to force actual execution
        if let Some(nudge) = planner.build_nudge_message() {
            return ReflectionOutcome::Nudge(format!(
                "CRITICAL: You just described what you would do but did NOT \
                 actually execute anything. The task plan has been generated — \
                 now you must EXECUTE each task step by step. Call the required \
                 tools NOW.\n\n{}",
                nudge
            ));
        }
        return ReflectionOutcome::Break;
    }

    // ── Normal completion-check path ──────────────────────────────────────────
    let mut made_progress = false;
    if let Some(ref content) = assistant_content {
        let completed_ids = planner.check_completion_in_content(content);
        let had_tool_calls = tool_calls_current_task > 0;
        let session_had_tools = total_tool_calls > 0;

        for completed_id in completed_ids {
            let task_needs_tool = planner.task_list.iter()
                .find(|t| t.id == completed_id)
                .map_or(false, |t| t.tool_hint.as_ref().map_or(false, |h| h != "analysis"));
            if task_needs_tool && !had_tool_calls && !session_had_tools {
                tracing::info!(
                    "Anti-hallucination: rejected text-only completion for task {} \
                     (requires tool but no tool calls made)",
                    completed_id
                );
                continue;
            }
            planner.mark_completed(completed_id);
            event_sink.on_task_progress(completed_id, true);
            made_progress = true;
        }
    }

    if planner.all_completed() {
        if let Some(ref content) = assistant_content {
            event_sink.on_text(content);
        }
        return ReflectionOutcome::AllDone;
    }

    if !made_progress {
        *consecutive_no_tool += 1;
    }

    if *consecutive_no_tool >= max_no_tool_retries {
        tracing::warn!("LLM failed to make progress after {} attempts, stopping", max_no_tool_retries);
        if let Some(ref content) = assistant_content {
            event_sink.on_text(content);
        }
        return ReflectionOutcome::Break;
    }

    if made_progress {
        return ReflectionOutcome::Continue;
    }

    // Auto-nudge: pending tasks remain, push LLM to continue
    if let Some(nudge) = planner.build_nudge_message() {
        tracing::info!("Auto-nudge (attempt {}): pending tasks remain, continuing", *consecutive_no_tool);
        return ReflectionOutcome::Nudge(nudge);
    }

    // No nudge available — emit and stop
    if let Some(ref content) = assistant_content {
        event_sink.on_text(content);
    }
    ReflectionOutcome::Break
}

// ── Post-tool completion check ────────────────────────────────────────────────

/// Check task completion claims in assistant content *after* tool execution.
/// Updates the planner and notifies the event sink.
pub(super) fn check_completion_after_tools(
    assistant_content: &Option<String>,
    planner: &mut TaskPlanner,
    event_sink: &mut dyn EventSink,
) {
    if let Some(ref content) = assistant_content {
        let completed_ids = planner.check_completion_in_content(content);
        let mut current_task_id = planner.current_task().map(|t| t.id);
        for completed_id in completed_ids {
            if let Some(cid) = current_task_id {
                if completed_id > cid {
                    tracing::info!(
                        "Anti-hallucination: ignoring premature completion for task {} \
                         (current task is {})",
                        completed_id, cid
                    );
                    continue;
                }
            }
            planner.mark_completed(completed_id);
            event_sink.on_task_progress(completed_id, true);
            current_task_id = planner.current_task().map(|t| t.id);
        }
    }
}

