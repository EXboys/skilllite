//! Reflection sub-module: hallucination detection, completion checking, auto-nudge.
//!
//! Handles every "no tool calls returned" scenario in the agent loop,
//! deciding whether to nudge the LLM, accept completion claims, or stop.

use super::super::task_planner::TaskPlanner;
use super::super::types::*;

// ── Outcome ───────────────────────────────────────────────────────────────────

/// What the caller should do after the reflection phase.
#[derive(Debug)]
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
        if messages.last().is_some_and(|m| m.role == "assistant") {
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
        if messages.last().is_some_and(|m| m.role == "assistant") {
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
                 tools NOW. Do NOT claim completion before calling `complete_task`, \
                 and do NOT say the overall job is finished while pending tasks remain.\n\n{}\n\nIf the plan does not fit the goal, you may call update_task_plan to revise it.",
                nudge
            ));
        }
        return ReflectionOutcome::Break;
    }

    // ── No-tool nudge path ────────────────────────────────────────────────────

    // Accept text response when all tasks done OR when the plan is empty
    // (empty plan = LLM decided no tools needed, e.g. conversational questions).
    // Without the is_empty check, empty-plan text gets popped from history
    // and the response is lost on reload ("Agent completed without text response").
    if planner.all_completed() || planner.is_empty() {
        if let Some(ref content) = assistant_content {
            event_sink.on_text(content);
        }
        return ReflectionOutcome::Break;
    }

    // Tasks still pending — swallow the premature summary so user never sees it
    if messages.last().is_some_and(|m| m.role == "assistant") {
        messages.pop();
    }
    tracing::info!("Swallowed no-tool assistant text while tasks are pending");

    *consecutive_no_tool += 1;
    if *consecutive_no_tool >= max_no_tool_retries {
        tracing::warn!(
            "LLM failed to make progress after {} attempts, stopping",
            max_no_tool_retries
        );
        return ReflectionOutcome::Break;
    }

    if let Some(nudge) = planner.build_nudge_message() {
        tracing::info!(
            "Auto-nudge (attempt {}): pending tasks remain",
            *consecutive_no_tool
        );
        return ReflectionOutcome::Nudge(nudge);
    }

    ReflectionOutcome::Break
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_planner::TaskPlanner;
    use crate::types::{ChatMessage, SilentEventSink, Task};

    fn planner_with_tasks(tasks: Vec<Task>) -> TaskPlanner {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = tasks;
        planner
    }

    #[test]
    fn test_reflect_simple_first_iteration_with_tools_nudges() {
        let mut no_tool_retries = 0;
        let mut messages = vec![
            ChatMessage::user("Do something"),
            ChatMessage::assistant("I will do it"),
        ];
        let mut sink = SilentEventSink;
        let content = Some("I will do it".to_string());

        let out = reflect_simple(
            &content,
            5,
            1,
            &mut no_tool_retries,
            3,
            &mut sink,
            &mut messages,
        );

        match &out {
            ReflectionOutcome::Nudge(s) => {
                assert!(s.contains("call the appropriate tool functions"));
                assert!(s.contains("Do not describe"));
            }
            _ => panic!("expected Nudge, got {:?}", out),
        }
        assert_eq!(no_tool_retries, 1);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_reflect_simple_break_when_has_content() {
        let mut no_tool_retries = 1;
        let mut messages = vec![];
        let mut sink = SilentEventSink;
        let content = Some("Here is the result".to_string());

        let out = reflect_simple(
            &content,
            5,
            2,
            &mut no_tool_retries,
            3,
            &mut sink,
            &mut messages,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
        assert_eq!(no_tool_retries, 2);
    }

    #[test]
    fn test_reflect_simple_break_when_max_retries() {
        let mut no_tool_retries = 2;
        let mut messages = vec![];
        let mut sink = SilentEventSink;
        let content = None;

        let out = reflect_simple(
            &content,
            5,
            3,
            &mut no_tool_retries,
            3,
            &mut sink,
            &mut messages,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
        assert_eq!(no_tool_retries, 3);
    }

    #[test]
    fn test_reflect_planning_all_completed_breaks() {
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Done".to_string(),
            tool_hint: None,
            completed: true,
        }]);
        let mut consecutive_no_tool = 0;
        let mut messages = vec![];
        let mut sink = SilentEventSink;
        let content = Some("All done!".to_string());

        let out = reflect_planning(
            &content,
            false,
            &mut planner,
            &mut consecutive_no_tool,
            3,
            &mut sink,
            &mut messages,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
    }

    #[test]
    fn test_reflect_planning_pending_tasks_nudges() {
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Generate a page".to_string(),
            tool_hint: Some("file_operation".to_string()),
            completed: false,
        }]);
        let mut consecutive_no_tool = 0;
        let mut messages = vec![ChatMessage::assistant("I'll summarize...")];
        let mut sink = SilentEventSink;
        let content = Some("I'll summarize...".to_string());

        let out = reflect_planning(
            &content,
            false,
            &mut planner,
            &mut consecutive_no_tool,
            3,
            &mut sink,
            &mut messages,
        );

        match &out {
            ReflectionOutcome::Nudge(s) => {
                assert!(s.contains("task") || s.contains("Task") || s.contains("pending"));
            }
            ReflectionOutcome::Break => {}
            _ => panic!("expected Nudge or Break, got {:?}", out),
        }
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_reflect_planning_empty_plan_preserves_response() {
        let mut planner = planner_with_tasks(vec![]);
        let mut consecutive_no_tool = 0;
        let mut messages = vec![ChatMessage::assistant("Here is the answer")];
        let mut sink = SilentEventSink;
        let content = Some("Here is the answer".to_string());

        let out = reflect_planning(
            &content,
            false,
            &mut planner,
            &mut consecutive_no_tool,
            3,
            &mut sink,
            &mut messages,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
        assert_eq!(
            messages.len(),
            1,
            "assistant message must NOT be popped for empty plans"
        );
    }
}
