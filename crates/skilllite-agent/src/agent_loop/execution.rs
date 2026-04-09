//! Execution sub-module: tool-call batch processing for the agent loop.
//!
//! Handles progressive disclosure, per-task call depth, update_task_plan
//! replan, failure tracking, and result processing for both simple and
//! task-planning loop paths.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use serde_json::Value;

use super::super::extensions::{self, MemoryVectorContext, PlanningControlExecutor};
use super::super::llm::LlmClient;
use super::super::skills::LoadedSkill;
use super::super::task_planner::TaskPlanner;
use super::super::types::*;
use super::helpers::{
    execute_tool_call, handle_complete_task, handle_update_task_plan,
    inject_progressive_disclosure, process_result_content,
};

/// Helper to get current timestamp string
fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

/// Append ToolCall **before** tool execution so transcript order matches UI:
/// `tool_call` → (optional `custom_message` confirmation/clarification during execute) → `tool_result`.
fn append_tool_call_to_transcript(
    session_key: Option<&str>,
    tool_call_id: &str,
    name: &str,
    arguments: &str,
) {
    let session_key = match session_key {
        Some(s) => s,
        None => return,
    };

    let chat_root = skilllite_executor::chat_root();
    let transcripts_dir = chat_root.join("transcripts");
    let t_path = match skilllite_executor::transcript::transcript_path_today(
        &transcripts_dir,
        session_key,
    ) {
        p if p
            .parent()
            .map(|p| skilllite_fs::create_dir_all(p).is_ok())
            .unwrap_or(false) =>
        {
            p
        }
        _ => return,
    };

    let now = timestamp_now();
    let tool_call_entry = skilllite_executor::transcript::TranscriptEntry::ToolCall {
        id: uuid::Uuid::new_v4().to_string(),
        parent_id: None,
        tool_call_id: tool_call_id.to_string(),
        name: name.to_string(),
        arguments: arguments.to_string(),
        timestamp: now,
    };
    let _ = skilllite_executor::transcript::append_entry(&t_path, &tool_call_entry);
}

/// Append ToolResult **after** tool execution (paired with [`append_tool_call_to_transcript`]).
fn append_tool_result_to_transcript(
    session_key: Option<&str>,
    tool_call_id: &str,
    name: &str,
    result: &str,
    is_error: bool,
    elapsed_ms: Option<u64>,
) {
    let session_key = match session_key {
        Some(s) => s,
        None => return,
    };

    let chat_root = skilllite_executor::chat_root();
    let transcripts_dir = chat_root.join("transcripts");
    let t_path = match skilllite_executor::transcript::transcript_path_today(
        &transcripts_dir,
        session_key,
    ) {
        p if p
            .parent()
            .map(|p| skilllite_fs::create_dir_all(p).is_ok())
            .unwrap_or(false) =>
        {
            p
        }
        _ => return,
    };

    let now = timestamp_now();
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
    /// Extra tool-call slots granted when the user chooses **Continue** on a
    /// `tool_call_limit` clarification (see agent loop). Base budget per turn is
    /// `max_iterations * max_tool_calls_per_task` (simple) or
    /// `effective_max * max_tool_calls_per_task` (planning); each approval adds
    /// another chunk of that size so the loop does not immediately re-trigger.
    pub tool_call_budget_extension: usize,
    pub failed_tool_calls: usize,
    pub consecutive_failures: usize,
    /// Calls since the last per-task depth reset.
    pub tool_calls_current_task: usize,
    pub replan_count: usize,
    pub tools_detail: Vec<ToolExecDetail>,
    pub context_overflow_retries: usize,
    pub iterations: usize,
    pub rules_used: Vec<String>,
    pub completion_type: TaskCompletionType,
    /// Tracks repeated failures: (tool_name, error_prefix) -> consecutive count.
    /// Reset when a different tool/error combination occurs or a tool succeeds.
    last_failure_sig: Option<(String, String)>,
    pub repeated_failure_count: usize,
    /// Cumulative LLM token usage (API-reported) for this agent run.
    pub llm_usage_totals: LlmUsageTotals,
}

const REPEATED_FAILURE_THRESHOLD: usize = 2;

/// After this many repeated complete_task failures, auto-complete the task
/// (only if the agent has executed at least one real tool, proving work was done).
const PLANNING_CONTROL_AUTO_RECOVER_THRESHOLD: usize = 3;

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            total_tool_calls: 0,
            tool_call_budget_extension: 0,
            failed_tool_calls: 0,
            consecutive_failures: 0,
            tool_calls_current_task: 0,
            replan_count: 0,
            tools_detail: Vec::new(),
            context_overflow_retries: 0,
            iterations: 0,
            rules_used: Vec::new(),
            completion_type: TaskCompletionType::Success,
            last_failure_sig: None,
            repeated_failure_count: 0,
            llm_usage_totals: LlmUsageTotals::default(),
        }
    }

    /// Record a tool failure and return the repeat count for this specific signature.
    pub fn record_failure(&mut self, tool_name: &str, error_content: &str) -> usize {
        let prefix: String = error_content.chars().take(80).collect();
        let sig = (tool_name.to_string(), prefix);
        if self.last_failure_sig.as_ref() == Some(&sig) {
            self.repeated_failure_count += 1;
        } else {
            self.last_failure_sig = Some(sig);
            self.repeated_failure_count = 1;
        }
        self.repeated_failure_count
    }

    pub fn reset_failure_sig(&mut self) {
        self.last_failure_sig = None;
        self.repeated_failure_count = 0;
    }

    pub fn observe_completion_type(&mut self, observed: TaskCompletionType) {
        match observed {
            TaskCompletionType::Failure => self.completion_type = TaskCompletionType::Failure,
            TaskCompletionType::PartialSuccess => {
                if self.completion_type != TaskCompletionType::Failure {
                    self.completion_type = TaskCompletionType::PartialSuccess;
                }
            }
            TaskCompletionType::Success => {}
        }
    }
}

fn parse_completion_type_from_arguments(arguments: &str) -> TaskCompletionType {
    let parsed: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(_) => return TaskCompletionType::Success,
    };
    let args = if let Some(inner) = parsed.as_str() {
        serde_json::from_str::<Value>(inner).unwrap_or(parsed)
    } else {
        parsed
    };
    match args
        .get("completion_type")
        .and_then(|v| v.as_str())
        .map(str::trim)
    {
        Some("partial_success") => TaskCompletionType::PartialSuccess,
        Some("failure") => TaskCompletionType::Failure,
        _ => TaskCompletionType::Success,
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

pub(super) fn should_suppress_planning_assistant_text(
    planner: &TaskPlanner,
    has_tool_calls: bool,
) -> bool {
    has_tool_calls && !planner.all_completed() && planner.current_task().is_some()
}

/// Soft upper limit on how many times a single session may call update_task_plan.
const MAX_REPLANS_PER_SESSION: usize = 3;

/// Executor for planning control tools, passed to registry.execute() in planning mode.
struct PlanningControlExecutorImpl<'a> {
    planner: &'a mut TaskPlanner,
    skills: &'a [LoadedSkill],
    state: &'a mut ExecutionState,
}

impl PlanningControlExecutor for PlanningControlExecutorImpl<'_> {
    fn execute(
        &mut self,
        tool_name: &str,
        arguments: &str,
        event_sink: &mut dyn super::super::types::EventSink,
    ) -> super::super::types::ToolResult {
        if tool_name == "update_task_plan" {
            self.state.replan_count += 1;
            let mut r = handle_update_task_plan(arguments, self.planner, self.skills, event_sink);
            if !r.is_error && self.state.replan_count >= MAX_REPLANS_PER_SESSION {
                r.content.push_str(&format!(
                    "\n\n⚠️ You have now replanned {} time(s). \
                     Please STOP replanning and EXECUTE the current plan step by step. \
                     Do NOT call update_task_plan again.",
                    self.state.replan_count
                ));
                tracing::info!(
                    "Replan soft limit reached ({}/{})",
                    self.state.replan_count,
                    MAX_REPLANS_PER_SESSION
                );
            }
            r
        } else if tool_name == "complete_task" {
            let declared = parse_completion_type_from_arguments(arguments);
            if declared == TaskCompletionType::Success
                && (self.state.failed_tool_calls > 0 || self.state.replan_count > 0)
            {
                return super::super::types::ToolResult {
                    tool_call_id: String::new(),
                    tool_name: "complete_task".to_string(),
                    content: "completion_type=success conflicts with execution signals \
                              (failed tools or replans observed). \
                              Use completion_type=partial_success or failure."
                        .to_string(),
                    is_error: true,
                    counts_as_failure: true,
                };
            }
            handle_complete_task(arguments, self.planner, event_sink)
        } else {
            super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Unknown planning control tool: {}", tool_name),
                is_error: true,
                counts_as_failure: true,
            }
        }
    }
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
        return ToolBatchOutcome {
            disclosure_injected: true,
            failure_limit_reached: false,
            depth_limit_reached: false,
        };
    }

    // After a complete_task triggers a task transition, all remaining tool
    // calls in this batch are skipped. This prevents the LLM from executing
    // work for future tasks without seeing the updated progress context that
    // the next iteration would inject (task focus message, nudge, etc.).
    let mut task_transitioned = false;

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);
        append_tool_call_to_transcript(session_key, &tc.id, tool_name, arguments);

        let is_planning_control =
            tool_name.as_str() == "update_task_plan" || tool_name.as_str() == "complete_task";

        if task_transitioned {
            tracing::info!(
                "Skipped post-transition tool call: {} (task already advanced, deferring to next iteration)",
                tool_name
            );
            let result = ToolResult {
                tool_call_id: tc.id.clone(),
                tool_name: tool_name.clone(),
                content: "Skipped: a task was just completed and the plan advanced. \
                          Continue with the next task in your next response."
                    .to_string(),
                is_error: false,
                counts_as_failure: false,
            };
            append_tool_result_to_transcript(
                session_key,
                &tc.id,
                tool_name,
                &result.content,
                false,
                None,
            );
            event_sink.on_tool_result(tool_name, &result.content, false);
            messages.push(ChatMessage::tool_result(
                &result.tool_call_id,
                &result.content,
            ));
            continue;
        }

        // Snapshot current task before execution to detect task transitions.
        let task_before = planner.current_task().map(|t| t.id);
        let start_time = Instant::now();
        let mut planning_executor = PlanningControlExecutorImpl {
            planner,
            skills,
            state,
        };
        let mut result = execute_tool_call(
            registry,
            tool_name,
            arguments,
            workspace,
            event_sink,
            embed_ctx,
            Some(&mut planning_executor),
        )
        .await;
        result.tool_call_id = tc.id.clone();
        result.content = process_result_content(client, model, tool_name, &result.content).await;

        if result.counts_as_failure {
            planning_executor.state.failed_tool_calls += 1;
            planning_executor.state.consecutive_failures += 1;
            let repeat_count = planning_executor
                .state
                .record_failure(tool_name, &result.content);

            if tool_name.as_str() == "complete_task"
                && repeat_count >= PLANNING_CONTROL_AUTO_RECOVER_THRESHOLD
                && planning_executor.state.total_tool_calls > 0
            {
                // Auto-complete only when the agent HAS executed real tools for this task
                // (i.e., actual work was done, just the completion signal is malformed).
                if let Some(current) = planning_executor.planner.current_task() {
                    let auto_id = current.id;
                    tracing::warn!(
                        "Auto-completing task {} after {} repeated complete_task failures \
                         (agent had {} successful tool calls)",
                        auto_id,
                        repeat_count,
                        planning_executor.state.total_tool_calls
                    );
                    planning_executor.planner.mark_completed(auto_id);
                    event_sink.on_task_progress(
                        auto_id,
                        true,
                        &planning_executor.planner.task_list,
                    );
                    result.is_error = false;
                    result.counts_as_failure = false;
                    result.content = format!(
                        "{{\"success\": true, \"task_id\": {}, \"completion_type\": \"success\", \"message\": \"Task {} auto-completed \
                         (format issues after {} attempts)\"}}",
                        auto_id, auto_id, repeat_count
                    );
                    planning_executor.state.consecutive_failures = 0;
                    planning_executor.state.reset_failure_sig();
                }
            } else if repeat_count >= REPEATED_FAILURE_THRESHOLD {
                result.content.push_str(&format!(
                    "\n\n⚠️ LOOP DETECTED: Tool '{}' has failed {} times in a row with the same error. \
                     You are stuck in a retry loop. STOP retrying this exact call. \
                     Instead: (1) re-read the error message carefully, (2) change the argument format, \
                     or (3) skip this step and proceed to the next task.",
                    tool_name, repeat_count
                ));
            } else if tool_name.as_str() != "complete_task" {
                result.content.push_str(
                    "\n\nTip: If this approach is wrong or the plan is no longer valid, \
                     call update_task_plan with a revised task list, then continue with the new plan."
                );
            }
        } else if !result.is_error {
            planning_executor.state.consecutive_failures = 0;
            planning_executor.state.reset_failure_sig();
        }
        // Detect task transition (via complete_task) and set cutoff flag.
        let task_after = planning_executor.planner.current_task().map(|t| t.id);
        if task_after != task_before {
            planning_executor.state.tool_calls_current_task = 0;
            task_transitioned = true;
        }
        if tool_name.as_str() == "complete_task" && !result.is_error {
            let completion_type = parse_completion_type_from_arguments(arguments);
            planning_executor
                .state
                .observe_completion_type(completion_type);
        }
        if !is_planning_control {
            planning_executor.state.tools_detail.push(ToolExecDetail {
                tool: tool_name.clone(),
                success: !result.is_error,
            });
        }

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        append_tool_result_to_transcript(
            session_key,
            &tc.id,
            tool_name,
            &result.content,
            result.is_error,
            Some(elapsed_ms),
        );

        registry.render_tool_result(tool_name, &result, event_sink);
        messages.push(ChatMessage::tool_result(
            &result.tool_call_id,
            &result.content,
        ));
        planning_executor.state.total_tool_calls += 1;
        planning_executor.state.tool_calls_current_task += 1;
    }

    let failure_limit_reached =
        max_consecutive_failures.is_some_and(|limit| state.consecutive_failures >= limit);
    let depth_limit_reached = state.tool_calls_current_task >= max_tool_calls_per_task;

    ToolBatchOutcome {
        disclosure_injected: false,
        failure_limit_reached,
        depth_limit_reached,
    }
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
        return ToolBatchOutcome {
            disclosure_injected: true,
            failure_limit_reached: false,
            depth_limit_reached: false,
        };
    }

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        let arguments = &tc.function.arguments;
        event_sink.on_tool_call(tool_name, arguments);
        append_tool_call_to_transcript(session_key, &tc.id, tool_name, arguments);

        let start_time = Instant::now();
        let mut result = execute_tool_call(
            registry, tool_name, arguments, workspace, event_sink, embed_ctx, None,
        )
        .await;
        result.tool_call_id = tc.id.clone();
        result.content = process_result_content(client, model, tool_name, &result.content).await;

        if result.counts_as_failure {
            state.failed_tool_calls += 1;
            state.consecutive_failures += 1;
            let repeat_count = state.record_failure(tool_name, &result.content);
            if repeat_count >= REPEATED_FAILURE_THRESHOLD {
                result.content.push_str(&format!(
                    "\n\n⚠️ LOOP DETECTED: Tool '{}' has failed {} times in a row with the same error. \
                     You are stuck in a retry loop. STOP retrying this exact call. \
                     Instead: (1) re-read the error message carefully, (2) change the argument format, \
                     or (3) try a completely different approach.",
                    tool_name, repeat_count
                ));
            }
        } else if !result.is_error {
            state.consecutive_failures = 0;
            state.reset_failure_sig();
        }
        state.tools_detail.push(ToolExecDetail {
            tool: tool_name.clone(),
            success: !result.is_error,
        });

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        append_tool_result_to_transcript(
            session_key,
            &tc.id,
            tool_name,
            &result.content,
            result.is_error,
            Some(elapsed_ms),
        );

        registry.render_tool_result(tool_name, &result, event_sink);
        messages.push(ChatMessage::tool_result(
            &result.tool_call_id,
            &result.content,
        ));
        state.total_tool_calls += 1;
    }

    let failure_limit_reached =
        max_consecutive_failures.is_some_and(|limit| state.consecutive_failures >= limit);
    ToolBatchOutcome {
        disclosure_injected: false,
        failure_limit_reached,
        depth_limit_reached: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::ExtensionRegistry;
    use crate::llm::LlmClient;
    use crate::types::{FunctionCall, SilentEventSink, Task, ToolCall};

    fn planner_with_tasks(tasks: Vec<Task>) -> TaskPlanner {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = tasks;
        planner
    }

    #[test]
    fn test_planning_assistant_text_suppressed_only_during_pending_tool_execution() {
        let pending = planner_with_tasks(vec![Task {
            id: 1,
            description: "Generate a page".to_string(),
            tool_hint: Some("file_operation".to_string()),
            completed: false,
        }]);
        assert!(should_suppress_planning_assistant_text(&pending, true));
        assert!(!should_suppress_planning_assistant_text(&pending, false));

        let done = planner_with_tasks(vec![Task {
            id: 1,
            description: "Generate a page".to_string(),
            tool_hint: Some("file_operation".to_string()),
            completed: true,
        }]);
        assert!(!should_suppress_planning_assistant_text(&done, true));
    }

    #[tokio::test]
    async fn test_execute_tool_batch_planning_allows_any_tool_for_current_task() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Start preview server and open in browser".to_string(),
            tool_hint: Some("preview".to_string()),
            completed: false,
        }]);
        let tool_calls = vec![
            ToolCall {
                id: "call_1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "file_exists".to_string(),
                    arguments: format!(
                        r#"{{"path":"{}"}}"#,
                        workspace.join("missing.txt").display()
                    ),
                },
            },
            ToolCall {
                id: "call_2".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "complete_task".to_string(),
                    arguments: r#"{"task_id":1,"summary":"done","completion_type":"success"}"#
                        .to_string(),
                },
            },
        ];
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();

        let outcome = execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "gemini-2.5-flash",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(3),
            None,
        )
        .await;

        assert!(!outcome.disclosure_injected);
        assert!(planner.task_list[0].completed);
        assert_eq!(planner.current_task().map(|t| t.id), None);
        assert_eq!(state.failed_tool_calls, 0);
    }

    #[tokio::test]
    async fn test_successful_tool_does_not_auto_complete_task_without_complete_task() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Write generated output".to_string(),
            tool_hint: Some("file_write".to_string()),
            completed: false,
        }]);
        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "write_file".to_string(),
                arguments: format!(
                    r#"{{"path":"{}","content":"hello"}}"#,
                    workspace.join("note.txt").display()
                ),
            },
        }];
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();

        let outcome = execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "gemini-2.5-flash",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(3),
            None,
        )
        .await;

        assert!(!outcome.disclosure_injected);
        assert!(!planner.task_list[0].completed);
        assert_eq!(planner.current_task().map(|t| t.id), Some(1));
        assert_eq!(state.failed_tool_calls, 0);
    }

    // ── Long-task regression: type coercion + loop detection ──────────────

    #[tokio::test]
    async fn test_complete_task_with_string_task_id_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Analyze data".to_string(),
            tool_hint: None,
            completed: false,
        }]);
        let tool_calls = vec![ToolCall {
            id: "call_ct".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "complete_task".to_string(),
                arguments: r#"{"task_id": "1", "summary": "done", "completion_type":"success"}"#
                    .to_string(),
            },
        }];
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();

        execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "test-model",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(3),
            None,
        )
        .await;

        assert!(
            planner.task_list[0].completed,
            "task_id as string \"1\" should complete the task"
        );
        assert_eq!(state.failed_tool_calls, 0);
    }

    #[tokio::test]
    async fn test_complete_task_rejects_success_when_failures_or_replans_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Analyze data".to_string(),
            tool_hint: None,
            completed: false,
        }]);
        let tool_calls = vec![ToolCall {
            id: "call_ct_guard".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "complete_task".to_string(),
                arguments: r#"{"task_id":1,"summary":"done","completion_type":"success"}"#
                    .to_string(),
            },
        }];
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();
        state.failed_tool_calls = 1;

        execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "test-model",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(3),
            None,
        )
        .await;

        assert!(
            !planner.task_list[0].completed,
            "success completion_type should be rejected when failure/replan signals exist"
        );
        assert!(
            state.failed_tool_calls >= 2,
            "guard rejection should count as failed tool call"
        );
    }

    #[tokio::test]
    async fn test_update_task_plan_with_stringified_tasks_array_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Old task".to_string(),
            tool_hint: None,
            completed: false,
        }]);
        let stringified_tasks =
            r#"{"tasks": "[{\"id\": 1, \"description\": \"New task\"}]", "reason": "replan"}"#;
        let tool_calls = vec![ToolCall {
            id: "call_utp".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "update_task_plan".to_string(),
                arguments: stringified_tasks.to_string(),
            },
        }];
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();

        execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "test-model",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(3),
            None,
        )
        .await;

        assert_eq!(
            state.failed_tool_calls, 0,
            "stringified tasks should be accepted"
        );
        assert!(
            planner
                .task_list
                .iter()
                .any(|t| t.description.contains("New task")),
            "new task should be in the plan"
        );
    }

    #[test]
    fn test_repeated_failure_detection() {
        let mut state = ExecutionState::new();

        let c1 = state.record_failure("complete_task", "Missing required field: task_id");
        assert_eq!(c1, 1);

        let c2 = state.record_failure("complete_task", "Missing required field: task_id");
        assert_eq!(c2, 2);
        assert!(c2 >= REPEATED_FAILURE_THRESHOLD);

        let c3 = state.record_failure("complete_task", "Missing required field: task_id");
        assert_eq!(c3, 3);
        assert!(c3 >= PLANNING_CONTROL_AUTO_RECOVER_THRESHOLD);

        state.reset_failure_sig();
        let c4 = state.record_failure("complete_task", "Missing required field: task_id");
        assert_eq!(c4, 1, "reset should restart the count");

        let c5 = state.record_failure("update_task_plan", "Missing or invalid 'tasks' array");
        assert_eq!(c5, 1, "different tool should restart the count");
    }

    #[tokio::test]
    async fn test_complete_task_auto_recovery_after_repeated_failures() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let registry = ExtensionRegistry::new(false, false, &[]);
        let client = LlmClient::new("", "").expect("test client");
        let mut planner = planner_with_tasks(vec![Task {
            id: 1,
            description: "Analyze data".to_string(),
            tool_hint: None,
            completed: false,
        }]);
        let mut sink = SilentEventSink;
        let mut messages = Vec::new();
        let mut documented_skills = HashSet::new();
        let mut state = ExecutionState::new();

        // Auto-recovery requires at least 1 real tool call (proves work was done)
        state.total_tool_calls = 3;

        // Third repeated complete_task format failure should trigger auto-recovery.
        let tool_calls = vec![
            ToolCall {
                id: "call_auto_1".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "complete_task".to_string(),
                    arguments: r#"{"task_id": 1}"#.to_string(),
                },
            },
            ToolCall {
                id: "call_auto_2".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "complete_task".to_string(),
                    arguments: r#"{"task_id": 1}"#.to_string(),
                },
            },
            ToolCall {
                id: "call_auto_3".to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "complete_task".to_string(),
                    arguments: r#"{"task_id": 1}"#.to_string(),
                },
            },
        ];

        execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            &mut sink,
            None,
            &client,
            "test-model",
            &mut planner,
            &[],
            &mut messages,
            &mut documented_skills,
            &mut state,
            8,
            Some(10),
            None,
        )
        .await;

        assert!(
            planner.task_list[0].completed,
            "task should be auto-completed after 3 repeated failures"
        );
        assert_eq!(
            state.consecutive_failures, 0,
            "consecutive failures should reset after auto-recovery"
        );
    }
}
