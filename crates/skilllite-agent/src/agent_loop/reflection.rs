//! Reflection sub-module: hallucination detection, completion checking, auto-nudge.
//!
//! Handles every "no tool calls returned" scenario in the agent loop,
//! deciding whether to nudge the LLM, accept completion claims, or stop.
//!
//! `reflect_simple` does NOT emit text (the simple loop always streams via `text_chunk`).
//! `reflect_planning` emits via [`crate::types::EventSink::emit_assistant_visible`] only
//! when streaming was suppressed (`suppress_stream=true`).

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
    /// Simple mode: text-only wrap-up after a successful tool batch — exit without user clarification.
    Complete,
    /// Planning mode: nudge without treating as a "stuck" iteration (e.g. remind `complete_task` after tools).
    SoftNudge(String),
}

// ── Simple-mode reflection ────────────────────────────────────────────────────

/// Reflect on a no-tool response in **simple mode**.
///
/// Handles the anti-hallucination nudge on iteration 1 and the normal
/// "too many no-tool retries" termination condition.
#[allow(clippy::too_many_arguments)]
pub(super) fn reflect_simple(
    assistant_content: &Option<String>,
    all_tools_len: usize,
    iterations: usize,
    no_tool_retries: &mut usize,
    max_no_tool_retries: usize,
    messages: &mut Vec<ChatMessage>,
    after_successful_tool_batch: bool,
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

    // After tools actually ran and succeeded, a text-only reply is usually a normal wrap-up — do not
    // route to `try_clarify` (which used to happen because `assistant_content.is_some()` forced Break).
    // Note: simple loop always uses streaming, so the text was already delivered via `text_chunk`
    // events. Do NOT call `emit_assistant_visible` here — the `done` event will finalize it.
    if after_successful_tool_batch
        && assistant_content
            .as_ref()
            .is_some_and(|c| !c.trim().is_empty())
    {
        return ReflectionOutcome::Complete;
    }

    // After tools succeeded, an empty (or whitespace-only) assistant reply must not end the
    // session silently — `Some("")` used to hit the Break branch below via `is_some()`.
    if after_successful_tool_batch
        && assistant_content
            .as_ref()
            .map(|c| c.trim().is_empty())
            .unwrap_or(true)
    {
        return ReflectionOutcome::SoftNudge(
            "Tools finished successfully, but you did not write a user-facing reply. \
             Write a concise closing summary now (2–6 sentences in the user's language): what was done, \
             the concrete outcome (URLs, paths, data, or errors), and optional next steps. \
             Do not call tools in this reply unless more work is clearly required."
                .to_string(),
        );
    }

    // Simple loop always uses streaming — text was already delivered via `text_chunk`.
    // Do NOT call `emit_assistant_visible` here; `done` finalizes the streaming message.

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
#[allow(clippy::too_many_arguments)]
pub(super) fn reflect_planning(
    assistant_content: &Option<String>,
    suppress_stream: bool,
    planner: &mut TaskPlanner,
    consecutive_no_tool: &mut usize,
    max_no_tool_retries: usize,
    event_sink: &mut dyn EventSink,
    messages: &mut Vec<ChatMessage>,
    after_successful_tool_batch: bool,
) -> ReflectionOutcome {
    if suppress_stream {
        // After a successful tool batch, text-only output is often a user-facing summary while the
        // model forgot `complete_task` — nudge structurally instead of popping as "hallucination".
        if after_successful_tool_batch
            && assistant_content
                .as_ref()
                .is_some_and(|c| !c.trim().is_empty())
        {
            // Do not `emit_assistant_visible` here: the model will usually answer again after
            // `complete_task` with streaming enabled, and a prior emit would duplicate the user-visible
            // summary (two assistant bubbles). The summary remains in `messages` for the next LLM turn.
            if let Some(ct) = planner.current_task() {
                let msg = format!(
                    "Tools ran successfully for the current step, but the plan is not updated yet. \
                     Call `complete_task(task_id={}, completion_type=\"success\"|\"partial_success\"|\"failure\")` now. \
                     If more tasks remain, continue with the next task; otherwise answer the user.",
                    ct.id
                );
                return ReflectionOutcome::SoftNudge(msg);
            }
        }
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
        // When `suppress_stream` is false, this LLM turn used streaming (`text_chunk`); the full
        // assistant body is already on the wire. Emitting `on_text` here duplicates the same
        // content for sinks that do not suppress (and can race RpcEventSink's `streamed_text`).
        if suppress_stream {
            if let Some(ref content) = assistant_content {
                event_sink.emit_assistant_visible(content);
            }
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
    use crate::types::{ChatMessage, ConfirmationRequest, SilentEventSink, Task};

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
        let content = Some("I will do it".to_string());

        let out = reflect_simple(
            &content,
            5,
            1,
            &mut no_tool_retries,
            3,
            &mut messages,
            false,
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
        let content = Some("Here is the result".to_string());

        let out = reflect_simple(
            &content,
            5,
            2,
            &mut no_tool_retries,
            3,
            &mut messages,
            false,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
        assert_eq!(no_tool_retries, 2);
    }

    #[test]
    fn test_reflect_simple_break_when_max_retries() {
        let mut no_tool_retries = 2;
        let mut messages = vec![];
        let content = None;

        let out = reflect_simple(
            &content,
            5,
            3,
            &mut no_tool_retries,
            3,
            &mut messages,
            false,
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
            false,
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
            false,
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
            false,
        );

        assert!(matches!(out, ReflectionOutcome::Break));
        assert_eq!(
            messages.len(),
            1,
            "assistant message must NOT be popped for empty plans"
        );
    }

    #[test]
    fn test_reflect_simple_complete_after_successful_tool_batch_with_text() {
        let mut no_tool_retries = 0;
        let mut messages = vec![];
        let content = Some("已清空记忆目录，请核实。".to_string());
        let out = reflect_simple(
            &content,
            5,
            2,
            &mut no_tool_retries,
            3,
            &mut messages,
            true,
        );
        assert!(matches!(out, ReflectionOutcome::Complete));
    }

    #[test]
    fn test_reflect_simple_soft_nudge_after_successful_tool_batch_empty_text() {
        let mut no_tool_retries = 0;
        let mut messages = vec![];
        for content in [Some(String::new()), Some("   ".to_string()), None] {
            let out = reflect_simple(
                &content,
                5,
                2,
                &mut no_tool_retries,
                3,
                &mut messages,
                true,
            );
            match &out {
                ReflectionOutcome::SoftNudge(s) => {
                    assert!(s.contains("closing summary"));
                    assert!(s.contains("Tools finished successfully"));
                }
                _ => panic!("expected SoftNudge, got {:?}", out),
            }
        }
    }

    #[test]
    fn test_reflect_planning_soft_nudge_after_tools_when_task_still_pending() {
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Clear memory".to_string(),
            tool_hint: Some("run_command".to_string()),
            completed: false,
        }]);
        let mut consecutive_no_tool = 0;
        let mut messages = vec![ChatMessage::assistant("命令已成功执行。")];
        let mut sink = SilentEventSink;
        let content = Some("命令已成功执行。".to_string());
        let out = reflect_planning(
            &content,
            true,
            &mut planner,
            &mut consecutive_no_tool,
            3,
            &mut sink,
            &mut messages,
            true,
        );
        match &out {
            ReflectionOutcome::SoftNudge(s) => {
                assert!(s.contains("complete_task"));
                assert!(s.contains("task_id=1"));
            }
            _ => panic!("expected SoftNudge, got {:?}", out),
        }
        assert_eq!(
            messages.len(),
            1,
            "assistant summary must not be popped on soft-nudge path"
        );
    }

    /// Ensures planning-mode soft-nudge does not push a user-visible assistant line; the next
    /// model turn (often streaming) should own the sole visible summary.
    struct CountAssistantVisible(u32);

    impl EventSink for CountAssistantVisible {
        fn emit_assistant_visible(&mut self, _text: &str) {
            self.0 += 1;
        }
        fn on_text(&mut self, _text: &str) {}
        fn on_tool_call(&mut self, _name: &str, _arguments: &str) {}
        fn on_tool_result(&mut self, _name: &str, _result: &str, _is_error: bool) {}
        fn on_confirmation_request(&mut self, _request: &ConfirmationRequest) -> bool {
            true
        }
    }

    #[test]
    fn test_reflect_planning_soft_nudge_does_not_emit_assistant_visible() {
        let mut planner = planner_with_tasks(vec![Task {
            id: 7,
            description: "Step".to_string(),
            tool_hint: Some("weather".to_string()),
            completed: false,
        }]);
        let mut consecutive_no_tool = 0;
        let mut messages = vec![ChatMessage::assistant("Summary text")];
        let mut sink = CountAssistantVisible(0);
        let content = Some("Summary text".to_string());
        let out = reflect_planning(
            &content,
            true,
            &mut planner,
            &mut consecutive_no_tool,
            3,
            &mut sink,
            &mut messages,
            true,
        );
        assert!(matches!(out, ReflectionOutcome::SoftNudge(_)));
        assert_eq!(
            sink.0, 0,
            "soft-nudge path must not emit visible assistant text"
        );
    }
}
