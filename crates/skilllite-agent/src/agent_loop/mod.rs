//! Core agent loop: LLM ↔ tool execution cycle.
//!
//! Phase 1: simple loop (no task planning).
//! Phase 2: task-planning-aware loop + run_command + LLM summarization.
//!
//! Ported from Python `AgenticLoop._run_openai`. Single implementation
//! that works for both CLI and RPC via the `EventSink` trait.
//!
//! Sub-modules:
//!   - `planning`       — pre-loop setup, LLM task-list generation, checkpoint saving
//!   - `execution`      — tool-call batch processing, progressive disclosure, depth limits
//!   - `reflection`     — no-tool response handling, hallucination guard, auto-nudge
//!   - `helpers`        — shared low-level utilities (tool execution, result processing, …)
//!   - `clarification`  — reusable clarification-request pattern
//!   - `llm_call`       — LLM call dispatch with context-overflow recovery

mod clarification;
mod execution;
mod helpers;
mod llm_call;
mod planning;
mod reflection;

use crate::Result;
use std::collections::HashSet;
use std::path::Path;

use super::extensions::{self, MemoryVectorContext};
use super::llm::LlmClient;
use super::prompt;
use super::skills::LoadedSkill;
use super::soul::Soul;
use super::types::*;
use skilllite_core::config::EmbeddingConfig;

use clarification::{
    no_progress_planning_copy, no_progress_simple_copy, too_many_failures_message, tool_limit_chip,
    try_clarify, ClarifyAction, CHIP_NARROW_SCOPE,
};
use execution::{
    execute_tool_batch_planning, execute_tool_batch_simple,
    should_suppress_planning_assistant_text, ExecutionState,
};
use helpers::build_agent_result;
use llm_call::{call_llm_with_recovery, LlmCallOutcome};
use planning::{
    build_task_focus_message, maybe_save_checkpoint, run_planning_phase, PlanningResult,
};
use reflection::{reflect_planning, reflect_simple, ReflectionOutcome};

fn strip_ansi_sequences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    enum State {
        Normal,
        Esc,
        Csi,
    }
    let mut state = State::Normal;
    for ch in s.chars() {
        match state {
            State::Normal => {
                if ch == '\u{1b}' {
                    state = State::Esc;
                } else {
                    out.push(ch);
                }
            }
            State::Esc => {
                if ch == '[' {
                    state = State::Csi;
                } else {
                    state = State::Normal;
                }
            }
            State::Csi => {
                if ('@'..='~').contains(&ch) {
                    state = State::Normal;
                }
            }
        }
    }
    out
}

fn collect_tool_result_excerpts(messages: &[ChatMessage], max_items: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut excerpts = Vec::new();
    for msg in messages.iter().rev() {
        if msg.role != "tool" {
            continue;
        }
        let raw = msg.content.as_deref().unwrap_or("").trim();
        if raw.is_empty() {
            continue;
        }
        let cleaned = strip_ansi_sequences(raw);
        let first_line = cleaned.lines().map(str::trim).find(|l| !l.is_empty());
        let Some(line) = first_line else {
            continue;
        };
        let one_line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        let excerpt = if one_line.len() > 140 {
            format!("{}…", safe_truncate(&one_line, 140))
        } else {
            one_line
        };
        if seen.insert(excerpt.clone()) {
            excerpts.push(excerpt);
            if excerpts.len() >= max_items {
                break;
            }
        }
    }
    excerpts
}

fn build_final_summary_fallback(
    task_plan: &[Task],
    messages: &[ChatMessage],
    user_message: &str,
) -> String {
    let total = task_plan.len();
    let completed = task_plan.iter().filter(|t| t.completed).count();
    let user_goal = if user_message.len() > 80 {
        format!("{}…", safe_truncate(user_message, 80))
    } else {
        user_message.to_string()
    };
    let mut lines = vec![
        format!("已完成你刚才的请求：{}", user_goal),
        format!("任务执行已结束，完成进度：{}/{}。", completed, total),
    ];

    let tool_excerpts = collect_tool_result_excerpts(messages, 3);
    if !tool_excerpts.is_empty() {
        lines.push("基于工具返回的关键信息：".to_string());
        for item in tool_excerpts {
            lines.push(format!("- {}", item));
        }
    } else {
        let completed_items: Vec<String> = task_plan
            .iter()
            .filter(|t| t.completed)
            .take(3)
            .map(|t| format!("{}: {}", t.id, t.description))
            .collect();
        if !completed_items.is_empty() {
            lines.push(format!(
                "已完成步骤（最多展示 3 条）：{}",
                completed_items.join("；")
            ));
        }
    }
    lines.push("如果你希望我继续，请补充下一步目标或细化要求。".to_string());
    lines.join("\n")
}

/// Run the agent loop.
///
/// Dispatches to either the simple loop (Phase 1) or the task-planning loop
/// (Phase 2) based on `config.enable_task_planning`.
pub async fn run_agent_loop(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    user_images: Option<Vec<UserImageAttachment>>,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    if config.enable_task_planning {
        run_with_task_planning(
            config,
            initial_messages,
            user_message,
            user_images,
            skills,
            event_sink,
            session_key,
        )
        .await
    } else {
        run_simple_loop(
            config,
            initial_messages,
            user_message,
            user_images,
            skills,
            event_sink,
            session_key,
        )
        .await
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Simple loop (Phase 1)
// ═══════════════════════════════════════════════════════════════════════════════

async fn run_simple_loop(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    user_images: Option<Vec<UserImageAttachment>>,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let start_time = std::time::Instant::now();
    let client = LlmClient::new(&config.api_base, &config.api_key)?;
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty()).then_some(
        MemoryVectorContext {
            client: &client,
            embed_config: &embed_config,
        },
    );

    let registry = if config.read_only_tools {
        extensions::ExtensionRegistry::read_only_with_task_planning(
            config.enable_memory,
            config.enable_memory_vector,
            config.enable_task_planning,
            skills,
        )
    } else {
        extensions::ExtensionRegistry::with_task_planning(
            config.enable_memory,
            config.enable_memory_vector,
            config.enable_task_planning,
            skills,
        )
    };
    let all_tools = registry.all_tool_definitions();

    let chat_root = skilllite_executor::chat_root();
    let soul = Soul::auto_load(config.soul_path.as_deref(), &config.workspace);
    let system_prompt = prompt::build_system_prompt(
        config.system_prompt.as_deref(),
        skills,
        &config.workspace,
        session_key,
        config.enable_memory,
        Some(registry.availability()),
        Some(&chat_root),
        soul.as_ref(),
        config.context_append.as_deref(),
    );
    let mut messages = Vec::new();
    messages.push(ChatMessage::system(&system_prompt));
    messages.extend(initial_messages);
    messages.push(ChatMessage::user_with_images(
        user_message,
        user_images.filter(|v| !v.is_empty()),
    ));

    let mut documented_skills: HashSet<String> = HashSet::new();
    let mut state = ExecutionState::new();
    let mut no_tool_retries = 0usize;
    let max_no_tool_retries = 3;
    let mut task_completed = true;
    let mut clarification_count = 0usize;
    // Set after a tool batch runs without disclosure/failure-limit and all new calls succeed.
    let mut after_successful_tool_batch = false;

    let tools_ref = if all_tools.is_empty() {
        None
    } else {
        Some(all_tools.as_slice())
    };

    loop {
        if state.iterations >= config.max_iterations {
            tracing::warn!(
                "Agent loop reached max iterations ({})",
                config.max_iterations
            );
            if let ClarifyAction::Continue = try_clarify(
                "max_iterations",
                &format!(
                    "已达到最大执行轮次 ({})，任务可能尚未完成。",
                    config.max_iterations
                ),
                &[CHIP_NARROW_SCOPE],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.iterations = 0;
                continue;
            }
            task_completed = false;
            break;
        }
        state.iterations += 1;

        // ── LLM call (with context-overflow recovery) ─────────────────────
        let response = match call_llm_with_recovery(
            &client,
            &config.model,
            &mut messages,
            tools_ref,
            config.temperature,
            true,
            event_sink,
            &mut state.context_overflow_retries,
        )
        .await?
        {
            LlmCallOutcome::Response(resp) => resp,
            LlmCallOutcome::Truncated => continue,
        };

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::validation("No choices in LLM response"))?;
        let assistant_content = choice.message.content;
        let tool_calls = choice.message.tool_calls;
        let has_tool_calls = tool_calls.as_ref().is_some_and(|tc| !tc.is_empty());

        if let Some(tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(
                assistant_content.as_deref(),
                tcs,
            ));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        // ── Reflection phase (no tool calls) ─────────────────────────────
        if !has_tool_calls {
            match reflect_simple(
                &assistant_content,
                all_tools.len(),
                state.iterations,
                &mut no_tool_retries,
                max_no_tool_retries,
                event_sink,
                &mut messages,
                after_successful_tool_batch,
            ) {
                ReflectionOutcome::Nudge(msg) => {
                    after_successful_tool_batch = false;
                    messages.push(ChatMessage::user(&msg));
                    continue;
                }
                ReflectionOutcome::Complete => {
                    break;
                }
                ReflectionOutcome::Break => {
                    after_successful_tool_batch = false;
                    let (clarify_msg, clarify_sugg) =
                        no_progress_simple_copy(state.iterations, no_tool_retries, all_tools.len());
                    let sugg_refs: Vec<&str> = clarify_sugg.iter().map(|s| s.as_str()).collect();
                    if let ClarifyAction::Continue = try_clarify(
                        "no_progress",
                        &clarify_msg,
                        &sugg_refs,
                        &mut clarification_count,
                        event_sink,
                        &mut messages,
                    ) {
                        no_tool_retries = 0;
                        continue;
                    }
                    break;
                }
                ReflectionOutcome::Continue => {
                    after_successful_tool_batch = false;
                    continue;
                }
                ReflectionOutcome::SoftNudge(msg) => {
                    after_successful_tool_batch = false;
                    messages.push(ChatMessage::user(&msg));
                    continue;
                }
            }
        }

        // ── Execution phase (tool calls present) ──────────────────────────
        no_tool_retries = 0;
        let tool_calls = match messages.last().and_then(|m| m.tool_calls.clone()) {
            Some(tc) if !tc.is_empty() => tc,
            _ => continue,
        };

        let tools_before = state.total_tool_calls;
        let outcome = execute_tool_batch_simple(
            &tool_calls,
            &registry,
            workspace,
            event_sink,
            embed_ctx.as_ref(),
            &client,
            &config.model,
            skills,
            &mut messages,
            &mut documented_skills,
            &mut state,
            config.max_consecutive_failures,
            session_key,
        )
        .await;

        if outcome.disclosure_injected {
            after_successful_tool_batch = false;
            continue;
        }
        if outcome.failure_limit_reached {
            after_successful_tool_batch = false;
            tracing::warn!(
                "Stopping: {} consecutive tool failures",
                state.consecutive_failures
            );
            let fail_msg =
                too_many_failures_message(state.consecutive_failures, &state.tools_detail);
            if let ClarifyAction::Continue = try_clarify(
                "too_many_failures",
                &fail_msg,
                &[
                    "先跳过受阻步骤，继续做后面互不依赖的待办",
                    "我补充环境信息（报错全文/路径/权限/期望输出）：",
                ],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.consecutive_failures = 0;
                continue;
            }
            task_completed = false;
            break;
        }

        let new_tools = state.total_tool_calls.saturating_sub(tools_before);
        after_successful_tool_batch = new_tools > 0 && state.consecutive_failures == 0;

        // 全局工具次数上限：`tool_call_budget_extension` 初值为 0 时，条件与原先
        // `total_tool_calls >= max_iterations * max_tool_calls_per_task` 完全相同。
        // 根因修复：用户在该处澄清里选「继续」后若不加 extension，下一轮会立刻再次命中
        // 同一条件，表现为「点了继续却没有继续」。每批准一次，追加一整块同名乘积额度。
        let base_tool_cap = config
            .max_iterations
            .saturating_mul(config.max_tool_calls_per_task);
        if state.total_tool_calls >= base_tool_cap.saturating_add(state.tool_call_budget_extension)
        {
            tracing::warn!("Agent loop reached total tool call limit");
            let tool_limit_msg = format!(
                "已达到工具调用次数上限（已累计 {} 次），任务可能尚未完成。",
                state.total_tool_calls
            );
            let tool_chip = tool_limit_chip(state.total_tool_calls);
            if let ClarifyAction::Continue = try_clarify(
                "tool_call_limit",
                &tool_limit_msg,
                &[tool_chip.as_str()],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.tool_call_budget_extension = state
                    .tool_call_budget_extension
                    .saturating_add(base_tool_cap);
                tracing::info!(
                    total_tool_calls = state.total_tool_calls,
                    base_tool_cap,
                    tool_call_budget_extension = state.tool_call_budget_extension,
                    "User chose continue after tool_call_limit; extended cap by one base chunk"
                );
                continue;
            }
            task_completed = false;
            break;
        }
    }

    let feedback = ExecutionFeedback {
        total_tools: state.total_tool_calls,
        failed_tools: state.failed_tool_calls,
        replans: 0,
        iterations: state.iterations,
        elapsed_ms: start_time.elapsed().as_millis() as u64,
        context_overflow_retries: state.context_overflow_retries,
        task_completed,
        completion_type: state.completion_type,
        task_description: Some(user_message.to_string()),
        rules_used: state.rules_used,
        tools_detail: state.tools_detail,
    };
    Ok(build_agent_result(
        messages,
        state.total_tool_calls,
        state.iterations,
        Vec::new(),
        feedback,
    ))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task-planning loop (Phase 2)
// ═══════════════════════════════════════════════════════════════════════════════

/// Agent loop with task planning: TaskPlanner + Auto-Nudge + per-task depth.
/// Uses planning / execution / reflection sub-modules as building blocks.
async fn run_with_task_planning(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    user_images: Option<Vec<UserImageAttachment>>,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let start_time = std::time::Instant::now();
    let client = LlmClient::new(&config.api_base, &config.api_key)?;
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty()).then_some(
        MemoryVectorContext {
            client: &client,
            embed_config: &embed_config,
        },
    );

    let registry = if config.read_only_tools {
        extensions::ExtensionRegistry::read_only_with_task_planning(
            config.enable_memory,
            config.enable_memory_vector,
            config.enable_task_planning,
            skills,
        )
    } else {
        extensions::ExtensionRegistry::with_task_planning(
            config.enable_memory,
            config.enable_memory_vector,
            config.enable_task_planning,
            skills,
        )
    };
    let all_tools = registry.all_tool_definitions();

    // ── Planning phase ─────────────────────────────────────────────────────
    let PlanningResult {
        mut planner,
        mut messages,
        chat_root,
        ..
    } = run_planning_phase(
        config,
        initial_messages,
        user_message,
        user_images,
        skills,
        registry.availability(),
        event_sink,
        session_key,
        &client,
        workspace,
    )
    .await?;

    let mut state = ExecutionState::new();
    let mut documented_skills: HashSet<String> = HashSet::new();
    let mut consecutive_no_tool = 0usize;
    let max_no_tool_retries = 3;
    let mut clarification_count = 0usize;
    let mut total_stuck_iterations = 0usize;
    const MAX_TOTAL_STUCK: usize = 8;
    let mut after_successful_tool_batch = false;

    let num_tasks = planner.task_list.len();
    let effective_max = if num_tasks == 0 {
        config.max_iterations
    } else {
        config
            .max_iterations
            .min((num_tasks * config.max_tool_calls_per_task).max(config.max_tool_calls_per_task))
    };

    let tools_ref = if all_tools.is_empty() {
        None
    } else {
        Some(all_tools.as_slice())
    };

    loop {
        if state.iterations >= effective_max {
            tracing::warn!(
                "Agent loop reached effective max iterations ({})",
                effective_max
            );
            if let ClarifyAction::Continue = try_clarify(
                "max_iterations",
                &format!("已达到最大执行轮次 ({})，任务可能尚未完成。", effective_max),
                &[CHIP_NARROW_SCOPE],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.iterations = 0;
                continue;
            }
            break;
        }
        state.iterations += 1;

        // Prevents premature summary text from leaking via streaming
        let suppress_stream = !planner.all_completed() && planner.current_task().is_some();

        // ── LLM call (with context-overflow recovery) ─────────────────────
        let response = match call_llm_with_recovery(
            &client,
            &config.model,
            &mut messages,
            tools_ref,
            config.temperature,
            !suppress_stream,
            event_sink,
            &mut state.context_overflow_retries,
        )
        .await?
        {
            LlmCallOutcome::Response(resp) => resp,
            LlmCallOutcome::Truncated => continue,
        };

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::validation("No choices in LLM response"))?;
        let mut assistant_content = choice.message.content;
        let tool_calls = choice.message.tool_calls;
        let has_tool_calls = tool_calls.as_ref().is_some_and(|tc| !tc.is_empty());
        let suppressed_planning_text =
            should_suppress_planning_assistant_text(&planner, has_tool_calls)
                && assistant_content
                    .as_ref()
                    .is_some_and(|content| !content.trim().is_empty());
        if suppressed_planning_text {
            tracing::info!("Suppressed free-form assistant text during pending task execution");
            assistant_content = None;
        }

        if let Some(tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(
                assistant_content.as_deref(),
                tcs,
            ));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        if suppress_stream && has_tool_calls {
            if let Some(ref content) = assistant_content {
                event_sink.on_text(content);
            }
        }

        // ── Reflection phase (no tool calls) ──────────────────────────────────
        if !has_tool_calls {
            match reflect_planning(
                &assistant_content,
                suppress_stream,
                &mut planner,
                &mut consecutive_no_tool,
                max_no_tool_retries,
                event_sink,
                &mut messages,
                after_successful_tool_batch,
            ) {
                ReflectionOutcome::Nudge(msg) => {
                    after_successful_tool_batch = false;
                    total_stuck_iterations += 1;
                    if total_stuck_iterations >= MAX_TOTAL_STUCK {
                        tracing::warn!(
                            "Combined stuck threshold reached ({} stuck iterations), \
                             breaking out of tool-fail/text-reject loop",
                            total_stuck_iterations
                        );
                        break;
                    }
                    messages.push(ChatMessage::user(&msg));
                    continue;
                }
                ReflectionOutcome::SoftNudge(msg) => {
                    after_successful_tool_batch = false;
                    messages.push(ChatMessage::user(&msg));
                    continue;
                }
                ReflectionOutcome::Break => {
                    after_successful_tool_batch = false;
                    // Empty plan: reflect_planning already treated text-only as normal completion
                    // (see `planner.is_empty()` branch). `all_completed()` is false for [], so we
                    // must not open clarification here — otherwise chit-chat loops on try_clarify.
                    if !planner.all_completed() && !planner.is_empty() {
                        let (clarify_msg, clarify_sugg) =
                            no_progress_planning_copy(&planner, consecutive_no_tool);
                        let sugg_refs: Vec<&str> =
                            clarify_sugg.iter().map(|s| s.as_str()).collect();
                        if let ClarifyAction::Continue = try_clarify(
                            "no_progress",
                            &clarify_msg,
                            &sugg_refs,
                            &mut clarification_count,
                            event_sink,
                            &mut messages,
                        ) {
                            consecutive_no_tool = 0;
                            total_stuck_iterations = 0;
                            continue;
                        }
                    }
                    break;
                }
                ReflectionOutcome::Continue | ReflectionOutcome::Complete => {
                    after_successful_tool_batch = false;
                    continue;
                }
            }
        }

        // ── Execution phase (tool calls present) ──────────────────────────────
        consecutive_no_tool = 0;
        let tool_calls = match messages.last().and_then(|m| m.tool_calls.clone()) {
            Some(tc) if !tc.is_empty() => tc,
            _ => continue,
        };

        let tools_before = state.total_tool_calls;
        let failures_before = state.failed_tool_calls;
        let outcome = execute_tool_batch_planning(
            &tool_calls,
            &registry,
            workspace,
            event_sink,
            embed_ctx.as_ref(),
            &client,
            &config.model,
            &mut planner,
            skills,
            &mut messages,
            &mut documented_skills,
            &mut state,
            config.max_tool_calls_per_task,
            config.max_consecutive_failures,
            session_key,
        )
        .await;

        let new_calls = state.total_tool_calls - tools_before;
        let new_failures = state.failed_tool_calls - failures_before;
        if new_calls > 0 && new_calls == new_failures {
            total_stuck_iterations += 1;
        } else {
            total_stuck_iterations = 0;
        }

        if total_stuck_iterations >= MAX_TOTAL_STUCK {
            tracing::warn!(
                "Combined stuck threshold reached ({} iterations), stopping loop",
                total_stuck_iterations
            );
            break;
        }

        if outcome.disclosure_injected {
            after_successful_tool_batch = false;
            continue;
        }
        if outcome.failure_limit_reached {
            after_successful_tool_batch = false;
            tracing::warn!(
                "Stopping: {} consecutive tool failures",
                state.consecutive_failures
            );
            let fail_msg =
                too_many_failures_message(state.consecutive_failures, &state.tools_detail);
            if let ClarifyAction::Continue = try_clarify(
                "too_many_failures",
                &fail_msg,
                &[
                    "先跳过受阻步骤，继续做后面互不依赖的待办",
                    "我补充环境信息（报错全文/路径/权限/期望输出）：",
                ],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.consecutive_failures = 0;
                continue;
            }
            break;
        }
        after_successful_tool_batch = new_calls > 0 && state.consecutive_failures == 0;
        if suppressed_planning_text && !planner.all_completed() {
            if let Some(nudge) = planner.build_nudge_message() {
                messages.push(ChatMessage::user(&format!(
                    "Pending tasks still exist. During execution, do not use free-form completion or wrap-up text. \
                     Complete the current task structurally with `complete_task`, then continue.\n\n{}",
                    nudge
                )));
            }
        }
        if outcome.depth_limit_reached {
            let depth_msg = planner.build_depth_limit_message(config.max_tool_calls_per_task);
            messages.push(ChatMessage::user(&depth_msg));
            state.tool_calls_current_task = 0;
        }

        // ── Post-tool completion check ─────────────────────────────────────────
        if planner.all_completed() {
            tracing::info!("All tasks completed, ending iteration");
            let has_substantial = assistant_content
                .as_ref()
                .is_some_and(|c| c.trim().len() > 50);
            if !has_substantial {
                let mut emitted_final_summary = false;
                match client
                    .chat_completion_stream(
                        &config.model,
                        &messages,
                        None,
                        config.temperature,
                        event_sink,
                    )
                    .await
                {
                    Ok(resp) => {
                        if let Some(ch) = resp.choices.into_iter().next() {
                            if let Some(ref content) = ch.message.content {
                                if !content.trim().is_empty() {
                                    event_sink.on_text(content);
                                    messages.push(ChatMessage::assistant(content));
                                    emitted_final_summary = true;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Final summary generation failed after completion: {}", e);
                    }
                }

                if !emitted_final_summary {
                    let fallback =
                        build_final_summary_fallback(&planner.task_list, &messages, user_message);
                    event_sink.on_text(&fallback);
                    messages.push(ChatMessage::assistant(&fallback));
                }
            }
            break;
        }

        maybe_save_checkpoint(
            session_key,
            user_message,
            config,
            &planner,
            &messages,
            &chat_root,
        );

        let tools_called: Vec<String> = {
            let mut seen = HashSet::new();
            let mut result = Vec::new();
            for d in state.tools_detail.iter().filter(|d| d.success) {
                if seen.insert(d.tool.as_str()) {
                    result.push(d.tool.clone());
                }
            }
            result
        };
        if let Some(focus_msg) = build_task_focus_message(&planner, &tools_called) {
            messages.push(ChatMessage::system(&focus_msg));
        }

        // 同简单循环：`extension==0` 时等价于原先 `effective_max * max_tool_calls_per_task`；
        // 「继续」后追加一块 `effective_max * max_tool_calls_per_task`，否则会继续误判上限。
        let base_tool_cap = effective_max.saturating_mul(config.max_tool_calls_per_task);
        if state.total_tool_calls >= base_tool_cap.saturating_add(state.tool_call_budget_extension)
        {
            tracing::warn!("Agent loop reached total tool call limit");
            let tool_limit_msg = format!(
                "已达到工具调用次数上限（已累计 {} 次），任务可能尚未完成。",
                state.total_tool_calls
            );
            let tool_chip = tool_limit_chip(state.total_tool_calls);
            if let ClarifyAction::Continue = try_clarify(
                "tool_call_limit",
                &tool_limit_msg,
                &[tool_chip.as_str()],
                &mut clarification_count,
                event_sink,
                &mut messages,
            ) {
                state.tool_call_budget_extension = state
                    .tool_call_budget_extension
                    .saturating_add(base_tool_cap);
                tracing::info!(
                    total_tool_calls = state.total_tool_calls,
                    base_tool_cap,
                    tool_call_budget_extension = state.tool_call_budget_extension,
                    "User chose continue after tool_call_limit (planning); extended cap by one base chunk"
                );
                continue;
            }
            break;
        }
    }

    let effective_completion_type = if planner.all_completed() {
        state.completion_type
    } else if state.total_tool_calls == 0
        || (state.failed_tool_calls > 0 && state.failed_tool_calls == state.total_tool_calls)
    {
        TaskCompletionType::Failure
    } else {
        TaskCompletionType::PartialSuccess
    };

    let feedback = ExecutionFeedback {
        total_tools: state.total_tool_calls,
        failed_tools: state.failed_tool_calls,
        replans: state.replan_count,
        iterations: state.iterations,
        elapsed_ms: start_time.elapsed().as_millis() as u64,
        context_overflow_retries: state.context_overflow_retries,
        task_completed: planner.all_completed(),
        completion_type: effective_completion_type,
        task_description: Some(user_message.to_string()),
        rules_used: planner.matched_rule_ids().to_vec(),
        tools_detail: state.tools_detail,
    };

    Ok(build_agent_result(
        messages,
        state.total_tool_calls,
        state.iterations,
        planner.task_list,
        feedback,
    ))
}

#[cfg(test)]
mod tests {
    use super::build_final_summary_fallback;
    use crate::types::{ChatMessage, Task};

    #[test]
    fn fallback_summary_includes_progress_and_completed_tasks() {
        let plan = vec![
            Task {
                id: 1,
                description: "打开 YouTube".to_string(),
                tool_hint: Some("agent_browser".to_string()),
                completed: true,
            },
            Task {
                id: 2,
                description: "搜索 AI 相关内容".to_string(),
                tool_hint: Some("agent_browser".to_string()),
                completed: true,
            },
        ];

        let text = build_final_summary_fallback(&plan, &[], "访问 YouTube 并搜索 AI");
        assert!(text.contains("2/2"));
        assert!(text.contains("打开 YouTube"));
        assert!(text.contains("搜索 AI 相关内容"));
    }

    #[test]
    fn tool_call_limit_user_extension_raises_ceiling_by_base_chunk() {
        let base_chunk = 50usize;
        let mut extension = 0usize;
        assert_eq!(base_chunk.saturating_add(extension), 50);
        let total_at_limit = 50usize;
        assert!(
            total_at_limit >= base_chunk.saturating_add(extension),
            "at-limit total should meet initial ceiling"
        );
        extension = extension.saturating_add(base_chunk);
        assert_eq!(base_chunk.saturating_add(extension), 100);
        assert!(
            total_at_limit < base_chunk.saturating_add(extension),
            "same total should be under raised ceiling until more tools run"
        );
    }

    #[test]
    fn fallback_summary_prefers_tool_result_excerpt_and_strips_ansi() {
        let plan = vec![Task {
            id: 1,
            description: "在 YouTube 搜索".to_string(),
            tool_hint: Some("agent_browser".to_string()),
            completed: true,
        }];
        let messages = vec![ChatMessage::tool_result(
            "call_1",
            "\u{1b}[32m✓\u{1b}[0m YouTube https://www.youtube.com/",
        )];
        let text = build_final_summary_fallback(&plan, &messages, "去 YouTube 搜索 AI");
        assert!(text.contains("YouTube https://www.youtube.com/"));
        assert!(!text.contains('\u{1b}'));
    }
}
