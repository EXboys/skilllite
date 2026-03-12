//! Reflection sub-module: hallucination detection, completion checking, auto-nudge.
//!
//! Handles every "no tool calls returned" scenario in the agent loop,
//! deciding whether to nudge the LLM, accept completion claims, or stop.

use super::super::task_planner::TaskPlanner;
use super::super::types::*;

// ── Outcome ───────────────────────────────────────────────────────────────────

/// What the caller should do after the reflection phase.
pub(super) enum ReflectionOutcome {
    /// Keep looping — no nudge needed (simple mode: made progress or empty first-pass).
    Continue,
    /// Inject this message as a user turn, then keep looping.
    Nudge(String),
    /// Stop the loop.
    Break,
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
/// Task completion is now handled exclusively via the `complete_task` tool call
/// (structured signal) or `try_auto_mark_task_on_success` (skill result parsing).
/// Text-based completion detection has been removed.
///
/// Two paths:
/// - `suppress_stream = true`:  hallucination-guard path (streaming was suppressed).
/// - `suppress_stream = false`: no-tool nudge path — pending tasks remain, push LLM to act.
pub(super) fn reflect_planning(
    assistant_content: &Option<String>,
    suppress_stream: bool,
    planner: &mut TaskPlanner,
    consecutive_no_tool: &mut usize,
    max_no_tool_retries: usize,
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
        if let Some(nudge) = planner.build_nudge_message() {
            return ReflectionOutcome::Nudge(format!(
                "CRITICAL: You just described what you would do but did NOT \
                 actually execute anything. The task plan has been generated — \
                 now you must EXECUTE each task step by step. Call the required \
                 tools NOW.\n\n{}\n\nIf the plan does not fit the goal, you may call update_task_plan to revise it.",
                nudge
            ));
        }
        return ReflectionOutcome::Break;
    }

    // ── No-tool nudge path ────────────────────────────────────────────────────
    // LLM returned only text (no tool calls, no complete_task). Pending tasks remain.
    // Emit the text and nudge LLM to call the appropriate tools / complete_task.
    if let Some(ref content) = assistant_content {
        event_sink.on_text(content);
    }

    *consecutive_no_tool += 1;
    if *consecutive_no_tool >= max_no_tool_retries {
        tracing::warn!("LLM failed to make progress after {} attempts, stopping", max_no_tool_retries);
        return ReflectionOutcome::Break;
    }

    if let Some(nudge) = planner.build_nudge_message() {
        tracing::info!("Auto-nudge (attempt {}): pending tasks remain", *consecutive_no_tool);
        return ReflectionOutcome::Nudge(nudge);
    }

    ReflectionOutcome::Break
}

