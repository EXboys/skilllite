//! Core agent loop: LLM ↔ tool execution cycle.
//!
//! Phase 1: simple loop (no task planning).
//! Phase 2: task-planning-aware loop + run_command + LLM summarization.
//!
//! Ported from Python `AgenticLoop._run_openai`. Single implementation
//! that works for both CLI and RPC via the `EventSink` trait.
//!
//! Sub-modules:
//!   - `planning`   — pre-loop setup, LLM task-list generation, checkpoint saving
//!   - `execution`  — tool-call batch processing, progressive disclosure, depth limits
//!   - `reflection` — no-tool response handling, hallucination guard, auto-nudge
//!   - `helpers`    — shared low-level utilities (tool execution, result processing, …)

mod helpers;
mod planning;
mod execution;
mod reflection;

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use super::extensions::{self, MemoryVectorContext};
use super::llm::{self, LlmClient};
use skilllite_core::config::EmbeddingConfig;
use super::prompt;
use super::skills::LoadedSkill;
use super::soul::Soul;
use super::types::*;

use helpers::build_agent_result;
use execution::{ExecutionState, execute_tool_batch_planning, execute_tool_batch_simple};
use planning::{PlanningResult, run_planning_phase, maybe_save_checkpoint, build_task_focus_message};
use reflection::{ReflectionOutcome, reflect_simple, reflect_planning, check_completion_after_tools};

/// Maximum number of context overflow recovery retries before giving up.
const MAX_CONTEXT_OVERFLOW_RETRIES: usize = 3;

/// Run the agent loop.
///
/// Dispatches to either the simple loop (Phase 1) or the task-planning loop
/// (Phase 2) based on `config.enable_task_planning`.
pub async fn run_agent_loop(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    if config.enable_task_planning {
        run_with_task_planning(config, initial_messages, user_message, skills, event_sink, session_key).await
    } else {
        run_simple_loop(config, initial_messages, user_message, skills, event_sink, session_key).await
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Simple loop (Phase 1)
// ═══════════════════════════════════════════════════════════════════════════════

async fn run_simple_loop(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let start_time = std::time::Instant::now();
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty())
        .then(|| MemoryVectorContext { client: &client, embed_config: &embed_config });

    let registry = extensions::ExtensionRegistry::new(
        config.enable_memory, config.enable_memory_vector, skills,
    );
    let all_tools = registry.all_tool_definitions();

    // Build system prompt and initial message list
    let chat_root = skilllite_executor::workspace_root(None)
        .unwrap_or_else(|_| {
            dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".skilllite")
        })
        .join("chat");
    let soul = Soul::auto_load(config.soul_path.as_deref(), &config.workspace);
    let system_prompt = prompt::build_system_prompt(
        config.system_prompt.as_deref(), skills, &config.workspace,
        session_key, config.enable_memory, Some(&chat_root), soul.as_ref(),
    );
    let mut messages = Vec::new();
    messages.push(ChatMessage::system(&system_prompt));
    messages.extend(initial_messages);
    messages.push(ChatMessage::user(user_message));

    let mut documented_skills: HashSet<String> = HashSet::new();
    let mut state = ExecutionState::new();
    let mut no_tool_retries = 0usize;
    let max_no_tool_retries = 3;

    let tools_ref = if all_tools.is_empty() { None } else { Some(all_tools.as_slice()) };

    loop {
        if state.iterations >= config.max_iterations {
            tracing::warn!("Agent loop reached max iterations ({})", config.max_iterations);
            break;
        }
        state.iterations += 1;

        // ── LLM call (with context-overflow recovery) ─────────────────────
        let response = match client
            .chat_completion_stream(&config.model, &messages, tools_ref, config.temperature, event_sink)
            .await
        {
            Ok(resp) => { state.context_overflow_retries = 0; resp }
            Err(e) => {
                if llm::is_context_overflow_error(&e.to_string()) {
                    state.context_overflow_retries += 1;
                    if state.context_overflow_retries >= MAX_CONTEXT_OVERFLOW_RETRIES {
                        tracing::error!("Context overflow persists after {} retries, giving up", MAX_CONTEXT_OVERFLOW_RETRIES);
                        return Err(e);
                    }
                    let rc = get_tool_result_recovery_max_chars();
                    tracing::warn!("Context overflow (attempt {}/{}), truncating to {} chars", state.context_overflow_retries, MAX_CONTEXT_OVERFLOW_RETRIES, rc);
                    llm::truncate_tool_messages(&mut messages, rc);
                    continue;
                }
                return Err(e);
            }
        };

        let choice = response.choices.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No choices in LLM response"))?;
        let assistant_content = choice.message.content.clone();
        let tool_calls = choice.message.tool_calls.clone();

        // Add assistant message to history
        if let Some(ref tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(assistant_content.as_deref(), tcs.clone()));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        let has_tool_calls = tool_calls.as_ref().map_or(false, |tc| !tc.is_empty());

        // ── Reflection phase (no tool calls) ─────────────────────────────
        if !has_tool_calls {
            match reflect_simple(
                &assistant_content, all_tools.len(), state.iterations,
                &mut no_tool_retries, max_no_tool_retries, event_sink, &mut messages,
            ) {
                ReflectionOutcome::Nudge(msg) => { messages.push(ChatMessage::user(&msg)); continue; }
                ReflectionOutcome::Break | ReflectionOutcome::AllDone => break,
                ReflectionOutcome::Continue => continue,
            }
        }

        // ── Execution phase (tool calls present) ──────────────────────────
        no_tool_retries = 0;
        let tool_calls = match tool_calls { Some(tc) => tc, None => continue };

        let outcome = execute_tool_batch_simple(
            &tool_calls, &registry, workspace, event_sink, embed_ctx.as_ref(),
            &client, &config.model, skills, &mut messages, &mut documented_skills,
            &mut state, config.max_consecutive_failures,
        ).await;

        if outcome.disclosure_injected { continue; }
        if outcome.failure_limit_reached {
            tracing::warn!("Stopping: {} consecutive tool failures", state.consecutive_failures);
            break;
        }

        // Global tool call depth limit
        if state.total_tool_calls >= config.max_iterations * config.max_tool_calls_per_task {
            tracing::warn!("Agent loop reached total tool call limit");
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
        task_completed: true,
        task_description: Some(user_message.to_string()),
        rules_used: Vec::new(),
        tools_detail: state.tools_detail,
    };
    Ok(build_agent_result(messages, state.total_tool_calls, state.iterations, Vec::new(), feedback))
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
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let start_time = std::time::Instant::now();
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty())
        .then(|| MemoryVectorContext { client: &client, embed_config: &embed_config });

    let registry = extensions::ExtensionRegistry::new(
        config.enable_memory, config.enable_memory_vector, skills,
    );
    let all_tools = registry.all_tool_definitions();

    // ── Planning phase ─────────────────────────────────────────────────────
    let PlanningResult { mut planner, mut messages, chat_root, .. } =
        run_planning_phase(config, initial_messages, user_message, skills, event_sink,
                           session_key, &client, workspace).await?;

    let mut state = ExecutionState::new();
    let mut documented_skills: HashSet<String> = HashSet::new();
    let mut consecutive_no_tool = 0usize;
    let max_no_tool_retries = 3;

    // Plan-based budget: min(global, num_tasks × per_task); fall back to max_iterations when empty
    let num_tasks = planner.task_list.len();
    let effective_max = if num_tasks == 0 { config.max_iterations }
                        else { config.max_iterations.min(num_tasks * config.max_tool_calls_per_task) };

    let tools_ref = if all_tools.is_empty() { None } else { Some(all_tools.as_slice()) };

    loop {
        if state.iterations >= effective_max {
            tracing::warn!("Agent loop reached effective max iterations ({})", effective_max);
            break;
        }
        state.iterations += 1;

        // ── Suppress streaming for hallucination-prone iterations ─────────────
        let suppress_stream = state.total_tool_calls == 0
            && planner.task_list.iter().any(|t| {
                !t.completed && t.tool_hint.as_ref().map_or(false, |h| h != "analysis")
            });

        // ── LLM call (with context-overflow recovery) ─────────────────────────
        let llm_result = if suppress_stream {
            client.chat_completion(&config.model, &messages, tools_ref, config.temperature).await
        } else {
            client.chat_completion_stream(&config.model, &messages, tools_ref, config.temperature, event_sink).await
        };

        let response = match llm_result {
            Ok(resp) => { state.context_overflow_retries = 0; resp }
            Err(e) => {
                if llm::is_context_overflow_error(&e.to_string()) {
                    state.context_overflow_retries += 1;
                    if state.context_overflow_retries >= MAX_CONTEXT_OVERFLOW_RETRIES {
                        tracing::error!("Context overflow persists after {} retries, giving up", MAX_CONTEXT_OVERFLOW_RETRIES);
                        return Err(e);
                    }
                    let rc = get_tool_result_recovery_max_chars();
                    tracing::warn!("Context overflow (attempt {}/{}), truncating to {} chars",
                                   state.context_overflow_retries, MAX_CONTEXT_OVERFLOW_RETRIES, rc);
                    llm::truncate_tool_messages(&mut messages, rc);
                    continue;
                }
                return Err(e);
            }
        };

        let choice = response.choices.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No choices in LLM response"))?;
        let assistant_content = choice.message.content.clone();
        let tool_calls = choice.message.tool_calls.clone();

        if let Some(ref tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(assistant_content.as_deref(), tcs.clone()));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        let has_tool_calls = tool_calls.as_ref().map_or(false, |tc| !tc.is_empty());

        // Emit suppressed text when LLM did return real tool calls (not a hallucination)
        if suppress_stream && has_tool_calls {
            if let Some(ref content) = assistant_content { event_sink.on_text(content); }
        }

        // ── Reflection phase (no tool calls) ──────────────────────────────────
        if !has_tool_calls {
            match reflect_planning(
                &assistant_content, suppress_stream, &mut planner,
                &mut consecutive_no_tool, max_no_tool_retries,
                state.tool_calls_current_task, state.total_tool_calls,
                event_sink, &mut messages,
            ) {
                ReflectionOutcome::Nudge(msg) => { messages.push(ChatMessage::user(&msg)); continue; }
                ReflectionOutcome::Continue => {
                    // Progress was made (task completed via text) — reset per-task counter
                    state.tool_calls_current_task = 0;
                    consecutive_no_tool = 0;
                    continue;
                }
                ReflectionOutcome::Break | ReflectionOutcome::AllDone => break,
            }
        }

        // ── Execution phase (tool calls present) ──────────────────────────────
        consecutive_no_tool = 0;
        let tool_calls = match tool_calls { Some(tc) => tc, None => continue };

        let outcome = execute_tool_batch_planning(
            &tool_calls, &registry, workspace, event_sink, embed_ctx.as_ref(),
            &client, &config.model, &mut planner, skills, &mut messages,
            &mut documented_skills, &mut state,
            config.max_tool_calls_per_task, config.max_consecutive_failures,
        ).await;

        if outcome.disclosure_injected { continue; }
        if outcome.failure_limit_reached {
            tracing::warn!("Stopping: {} consecutive tool failures", state.consecutive_failures);
            break;
        }
        if outcome.depth_limit_reached {
            let depth_msg = planner.build_depth_limit_message(config.max_tool_calls_per_task);
            messages.push(ChatMessage::user(&depth_msg));
            state.tool_calls_current_task = 0; // reset so next task gets its full quota
        }

        // ── Post-tool completion check ─────────────────────────────────────────
        check_completion_after_tools(&assistant_content, &mut planner, event_sink);

        if planner.all_completed() {
            tracing::info!("All tasks completed, ending iteration");
            let has_substantial = assistant_content.as_ref().map_or(false, |c| c.trim().len() > 50);
            if !has_substantial {
                if let Ok(resp) = client.chat_completion_stream(
                    &config.model, &messages, None, config.temperature, event_sink,
                ).await {
                    if let Some(ch) = resp.choices.into_iter().next() {
                        if let Some(ref content) = ch.message.content {
                            event_sink.on_text(content);
                            messages.push(ChatMessage::assistant(content));
                        }
                    }
                }
            }
            break;
        }

        // A13: Per-iteration checkpoint (run mode) for --resume
        maybe_save_checkpoint(session_key, user_message, config, &planner, &messages, &chat_root);

        // Task focus: inject progress update as system message
        if let Some(focus_msg) = build_task_focus_message(&planner) {
            messages.push(ChatMessage::system(&focus_msg));
        }

        // Global tool call depth limit
        if state.total_tool_calls >= effective_max * config.max_tool_calls_per_task {
            tracing::warn!("Agent loop reached total tool call limit");
            break;
        }
    }

    let feedback = ExecutionFeedback {
        total_tools: state.total_tool_calls,
        failed_tools: state.failed_tool_calls,
        replans: state.replan_count,
        iterations: state.iterations,
        elapsed_ms: start_time.elapsed().as_millis() as u64,
        context_overflow_retries: state.context_overflow_retries,
        task_completed: planner.all_completed(),
        task_description: Some(user_message.to_string()),
        rules_used: Vec::new(),
        tools_detail: state.tools_detail,
    };

    Ok(build_agent_result(messages, state.total_tool_calls, state.iterations, planner.task_list, feedback))
}

