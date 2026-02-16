//! Core agent loop: LLM ↔ tool execution cycle.
//!
//! Phase 1: simple loop (no task planning).
//! Phase 2: task-planning-aware loop + run_command + LLM summarization.
//!
//! Ported from Python `AgenticLoop._run_openai`. Single implementation
//! that works for both CLI and RPC via the `EventSink` trait.
//!
//! Two code paths, selected by `config.enable_task_planning`:
//!   - `run_simple_loop`: original Phase 1 logic, nearly unchanged
//!   - `run_with_task_planning`: Phase 2 with TaskPlanner, Auto-Nudge, etc.

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use super::llm::{self, LlmClient};
use super::long_text;
use super::prompt;
use super::skills::{self, LoadedSkill};
use super::task_planner::TaskPlanner;
use super::tools;
use super::types::*;

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
) -> Result<AgentResult> {
    if config.enable_task_planning {
        run_with_task_planning(config, initial_messages, user_message, skills, event_sink).await
    } else {
        run_simple_loop(config, initial_messages, user_message, skills, event_sink).await
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Simple loop (Phase 1 — original logic, enhanced for Phase 2 tools)
// ═══════════════════════════════════════════════════════════════════════════════

/// Original agent loop: no task planning.
/// Nearly identical to Phase 1, with Phase 2 enhancements:
///   - run_command / write_output / preview_server support
///   - env-configurable context overflow recovery chars
///   - LLM summarization for very long tool results
async fn run_simple_loop(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
) -> Result<AgentResult> {
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);

    // Collect all tool definitions: built-in + skills
    let mut all_tools = tools::get_builtin_tool_definitions();
    for skill in skills {
        all_tools.extend(skill.tool_definitions.clone());
    }

    // Build system prompt
    let system_prompt = prompt::build_system_prompt(
        config.system_prompt.as_deref(),
        skills,
        &config.workspace,
    );

    // Build message list
    let mut messages = Vec::new();
    messages.push(ChatMessage::system(&system_prompt));
    messages.extend(initial_messages);
    messages.push(ChatMessage::user(user_message));

    // Progressive disclosure: track which skills have had full docs injected
    let mut documented_skills: HashSet<String> = HashSet::new();

    let mut total_tool_calls = 0usize;
    let mut iterations = 0usize;
    let mut no_tool_retries = 0usize;
    let max_no_tool_retries = 3;

    let tools_ref = if all_tools.is_empty() {
        None
    } else {
        Some(all_tools.as_slice())
    };

    loop {
        if iterations >= config.max_iterations {
            tracing::warn!("Agent loop reached max iterations ({})", config.max_iterations);
            break;
        }
        iterations += 1;

        // Call LLM
        let response = match client
            .chat_completion_stream(
                &config.model,
                &messages,
                tools_ref,
                config.temperature,
                event_sink,
            )
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let err_msg = e.to_string();
                if llm::is_context_overflow_error(&err_msg) {
                    // Context overflow recovery: truncate tool messages and retry
                    let recovery_chars = get_tool_result_recovery_max_chars();
                    tracing::warn!(
                        "Context overflow, truncating tool messages to {} chars and retrying",
                        recovery_chars
                    );
                    llm::truncate_tool_messages(&mut messages, recovery_chars);
                    continue;
                }
                return Err(e);
            }
        };

        // Extract the first choice
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No choices in LLM response"))?;

        let assistant_content = choice.message.content.clone();
        let tool_calls = choice.message.tool_calls.clone();

        // Add assistant message to history
        if let Some(ref tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(
                assistant_content.as_deref(),
                tcs.clone(),
            ));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        // No tool calls: check if we should continue or stop
        if tool_calls.is_none() || tool_calls.as_ref().map_or(true, |tc| tc.is_empty()) {
            // Anti-hallucination nudge: if this is the first iteration and
            // tools are available but the LLM chose not to call any, nudge
            // it once.  This catches the common pattern where the model
            // fabricates a "completed" response instead of actually invoking
            // tools.  No content inspection needed — the structural signal
            // (tools available + none called on turn 1) is sufficient.
            if iterations == 1 && !all_tools.is_empty() && no_tool_retries == 0 {
                tracing::info!("First iteration produced no tool calls with {} tools available, nudging", all_tools.len());
                // Remove the text-only assistant message so it doesn't
                // pollute the conversation history.
                if let Some(last) = messages.last() {
                    if last.role == "assistant" {
                        messages.pop();
                    }
                }
                messages.push(ChatMessage::user(
                    "You responded with text but did not call any tools. \
                     If the task requires action (browsing, file I/O, computation, etc.), \
                     you MUST call the appropriate tool functions. Do not describe what you \
                     would do — actually do it by invoking the tools."
                ));
                no_tool_retries += 1;
                continue;
            }

            // Notify event sink of final text
            if let Some(ref content) = assistant_content {
                event_sink.on_text(content);
            }

            no_tool_retries += 1;
            if no_tool_retries >= max_no_tool_retries {
                break;
            }

            if assistant_content.is_some() {
                break;
            }

            continue;
        }

        // Process tool calls
        no_tool_retries = 0;
        let tool_calls = tool_calls.unwrap();

        // Progressive disclosure
        if inject_progressive_disclosure(
            &tool_calls,
            skills,
            &mut documented_skills,
            &mut messages,
        ) {
            continue;
        }

        // Execute each tool call
        for tc in &tool_calls {
            let tool_name = &tc.function.name;
            let arguments = &tc.function.arguments;

            event_sink.on_tool_call(tool_name, arguments);

            let mut result =
                execute_tool_call(tool_name, arguments, workspace, skills, event_sink).await;
            result.tool_call_id = tc.id.clone();

            // Process long content (sync fast path → async LLM summarization fallback)
            result.content =
                process_result_content(&client, &config.model, &result.content).await;

            event_sink.on_tool_result(tool_name, &result.content, result.is_error);
            messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));

            total_tool_calls += 1;
        }

        // Check tool call depth limit
        if total_tool_calls >= config.max_iterations * config.max_tool_calls_per_task {
            tracing::warn!("Agent loop reached total tool call limit");
            break;
        }
    }

    Ok(build_agent_result(messages, total_tool_calls, iterations))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task-planning loop (Phase 2)
// ═══════════════════════════════════════════════════════════════════════════════

/// Agent loop with task planning: TaskPlanner + Auto-Nudge + per-task depth.
/// Ported from Python `AgenticLoop._run_openai` with task planning enabled.
async fn run_with_task_planning(
    config: &AgentConfig,
    initial_messages: Vec<ChatMessage>,
    user_message: &str,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
) -> Result<AgentResult> {
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);

    // Collect all tool definitions: built-in + skills
    let mut all_tools = tools::get_builtin_tool_definitions();
    for skill in skills {
        all_tools.extend(skill.tool_definitions.clone());
    }

    // ── Task planning ──────────────────────────────────────────────────────
    let mut planner = TaskPlanner::new();

    // Build conversation context from initial_messages (for "继续" detection)
    let conversation_context: Option<String> = if !initial_messages.is_empty() {
        let ctx: Vec<String> = initial_messages
            .iter()
            .filter_map(|m| {
                m.content
                    .as_ref()
                    .map(|c| format!("[{}] {}", m.role, c))
            })
            .collect();
        if ctx.is_empty() {
            None
        } else {
            Some(ctx.join("\n"))
        }
    } else {
        None
    };

    // Generate task list via LLM
    let _tasks = planner
        .generate_task_list(
            &client,
            &config.model,
            user_message,
            skills,
            conversation_context.as_deref(),
        )
        .await?;

    // Notify event sink of the plan
    event_sink.on_task_plan(&planner.task_list);

    // ── Empty plan: LLM answers directly, NO tools ──────────────────────
    // Matches Python SDK: `_no_tools_needed = True` + `enable_task_planning = False`.
    // When the planner says no tools are needed, we don't even pass tools
    // to the LLM — it can only generate text. This structurally prevents
    // hallucinated tool calls.
    if planner.is_empty() {
        tracing::info!(
            "TaskPlanner returned empty list — LLM will answer directly without tools"
        );
        let system_prompt = prompt::build_system_prompt(
            config.system_prompt.as_deref(),
            skills,
            &config.workspace,
        );
        let mut messages = Vec::new();
        messages.push(ChatMessage::system(&system_prompt));
        messages.extend(initial_messages);
        messages.push(ChatMessage::user(user_message));

        let response = client
            .chat_completion_stream(
                &config.model,
                &messages,
                None, // no tools
                config.temperature,
                event_sink,
            )
            .await?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No choices in LLM response"))?;
        if let Some(ref content) = choice.message.content {
            event_sink.on_text(content);
        }
        messages.push(ChatMessage::assistant(
            choice.message.content.as_deref().unwrap_or(""),
        ));

        return Ok(build_agent_result(messages, 0, 1));
    }

    // ── Non-empty plan: plan-driven execution ───────────────────────────
    // Build task-aware system prompt and run with tools + nudge mechanism.
    // Matches Python SDK `_run_openai` when `enable_task_planning = True`.

    let system_prompt = planner.build_task_system_prompt(skills);

    let mut messages = Vec::new();
    messages.push(ChatMessage::system(&system_prompt));
    messages.extend(initial_messages);
    messages.push(ChatMessage::user(user_message));

    let mut documented_skills: HashSet<String> = HashSet::new();
    let mut total_tool_calls = 0usize;
    let mut iterations = 0usize;
    let mut consecutive_no_tool = 0usize;
    let max_no_tool_retries = 3;
    let mut tool_calls_current_task = 0usize;

    // Plan-based budget: effective_max = min(global, num_tasks * per_task)
    let num_tasks = planner.task_list.len();
    let effective_max = config
        .max_iterations
        .min(num_tasks * config.max_tool_calls_per_task);

    let tools_ref = if all_tools.is_empty() {
        None
    } else {
        Some(all_tools.as_slice())
    };

    loop {
        if iterations >= effective_max {
            tracing::warn!(
                "Agent loop reached effective max iterations ({})",
                effective_max
            );
            break;
        }
        iterations += 1;

        // Call LLM
        let response = match client
            .chat_completion_stream(
                &config.model,
                &messages,
                tools_ref,
                config.temperature,
                event_sink,
            )
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                let err_msg = e.to_string();
                if llm::is_context_overflow_error(&err_msg) {
                    let recovery_chars = get_tool_result_recovery_max_chars();
                    tracing::warn!(
                        "Context overflow, truncating tool messages to {} chars and retrying",
                        recovery_chars
                    );
                    llm::truncate_tool_messages(&mut messages, recovery_chars);
                    continue;
                }
                return Err(e);
            }
        };

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No choices in LLM response"))?;

        let assistant_content = choice.message.content.clone();
        let tool_calls = choice.message.tool_calls.clone();

        // Add assistant message to history
        if let Some(ref tcs) = tool_calls {
            messages.push(ChatMessage::assistant_with_tool_calls(
                assistant_content.as_deref(),
                tcs.clone(),
            ));
        } else if let Some(ref content) = assistant_content {
            messages.push(ChatMessage::assistant(content));
        }

        // ── No tool calls: check plan progress, nudge if needed ─────────
        // Matches Python SDK lines 592-645.
        let has_tool_calls = tool_calls
            .as_ref()
            .map_or(false, |tc| !tc.is_empty());

        if !has_tool_calls {
            // Check if a task was completed (text-based, per plan)
            let mut made_progress = false;
            if let Some(ref content) = assistant_content {
                if let Some(completed_id) = planner.check_completion_in_content(content) {
                    planner.mark_completed(completed_id);
                    event_sink.on_task_progress(completed_id, true);
                    consecutive_no_tool = 0;
                    tool_calls_current_task = 0;
                    made_progress = true;
                }
            }

            // All tasks done → finish
            if planner.all_completed() {
                if let Some(ref content) = assistant_content {
                    event_sink.on_text(content);
                }
                tracing::info!("All tasks completed, ending iteration");
                break;
            }

            // Tasks remain but no progress → nudge or bail
            if !made_progress {
                consecutive_no_tool += 1;
            }

            if consecutive_no_tool >= max_no_tool_retries {
                tracing::warn!(
                    "LLM failed to make progress after {} attempts, stopping",
                    max_no_tool_retries
                );
                if let Some(ref content) = assistant_content {
                    event_sink.on_text(content);
                }
                break;
            }

            // Auto-nudge: pending tasks remain, push LLM to continue
            if let Some(nudge) = planner.build_nudge_message() {
                tracing::info!(
                    "Auto-nudge (attempt {}): pending tasks remain, continuing",
                    consecutive_no_tool
                );
                messages.push(ChatMessage::user(&nudge));
                continue;
            }

            // No nudge available (shouldn't happen), emit and break
            if let Some(ref content) = assistant_content {
                event_sink.on_text(content);
            }
            break;
        }

        // ── Process tool calls ─────────────────────────────────────────────
        consecutive_no_tool = 0;
        let tool_calls = tool_calls.unwrap();

        // Progressive disclosure
        if inject_progressive_disclosure(
            &tool_calls,
            skills,
            &mut documented_skills,
            &mut messages,
        ) {
            continue;
        }

        // Execute each tool call
        for tc in &tool_calls {
            let tool_name = &tc.function.name;
            let arguments = &tc.function.arguments;

            event_sink.on_tool_call(tool_name, arguments);

            let mut result =
                execute_tool_call(tool_name, arguments, workspace, skills, event_sink).await;
            result.tool_call_id = tc.id.clone();

            result.content =
                process_result_content(&client, &config.model, &result.content).await;

            event_sink.on_tool_result(tool_name, &result.content, result.is_error);
            messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));

            total_tool_calls += 1;
            tool_calls_current_task += 1;
        }

        // ── Per-task depth limit ───────────────────────────────────────────
        if tool_calls_current_task >= config.max_tool_calls_per_task {
            let depth_msg = planner.build_depth_limit_message(config.max_tool_calls_per_task);
            messages.push(ChatMessage::user(&depth_msg));
            tracing::debug!(
                "Per-task depth limit reached ({} calls), requesting summary",
                config.max_tool_calls_per_task
            );
        }

        // ── Check task completion after tool execution ───────────────────
        // Matches Python SDK lines 718-771.
        if let Some(ref content) = assistant_content {
            if let Some(completed_id) = planner.check_completion_in_content(content) {
                planner.mark_completed(completed_id);
                event_sink.on_task_progress(completed_id, true);
                tool_calls_current_task = 0;
            }
        }

        if planner.all_completed() {
            tracing::info!("All tasks completed, ending iteration");
            // All done — get final summary from LLM (without tools)
            let final_response = client
                .chat_completion_stream(
                    &config.model,
                    &messages,
                    None, // no tools for final summary
                    config.temperature,
                    event_sink,
                )
                .await;
            if let Ok(resp) = final_response {
                if let Some(choice) = resp.choices.into_iter().next() {
                    if let Some(ref content) = choice.message.content {
                        event_sink.on_text(content);
                        messages.push(ChatMessage::assistant(content));
                    }
                }
            }
            break;
        }

        // Update task focus: inject task progress as system message
        if let Some(current) = planner.current_task() {
            let task_list_json =
                serde_json::to_string_pretty(&planner.task_list)
                    .unwrap_or_else(|_| "[]".to_string());
            let tool_hint = current.tool_hint.as_deref().unwrap_or("");
            let focus_msg = if !tool_hint.is_empty()
                && tool_hint != "file_operation"
                && tool_hint != "analysis"
            {
                format!(
                    "Task progress update:\n{}\n\n\
                     Current task to execute: Task {} - {}\n\n\
                     ⚡ Call `{}` DIRECTLY. Do NOT explore files first.",
                    task_list_json, current.id, current.description, tool_hint
                )
            } else {
                format!(
                    "Task progress update:\n{}\n\n\
                     Current task to execute: Task {} - {}\n\n\
                     Please continue to focus on completing the current task.",
                    task_list_json, current.id, current.description
                )
            };
            messages.push(ChatMessage::system(&focus_msg));
        }

        // Total tool call limit
        if total_tool_calls >= effective_max * config.max_tool_calls_per_task {
            tracing::warn!("Agent loop reached total tool call limit");
            break;
        }
    }

    Ok(build_agent_result(messages, total_tool_calls, iterations))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Execute a single tool call (built-in sync, built-in async, or skill).
async fn execute_tool_call(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
) -> ToolResult {
    if tools::is_builtin_tool(tool_name) {
        if tools::is_async_builtin_tool(tool_name) {
            // Async built-in (run_command, preview_server)
            tools::execute_async_builtin_tool(tool_name, arguments, workspace, event_sink).await
        } else {
            // Sync built-in (read_file, write_file, etc.)
            tools::execute_builtin_tool(tool_name, arguments, workspace)
        }
    } else if let Some(skill) = skills::find_skill_by_tool_name(skills, tool_name) {
        // Skill tool
        skills::execute_skill(skill, tool_name, arguments, workspace, event_sink)
    } else {
        ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Unknown tool: {}", tool_name),
            is_error: true,
        }
    }
}

/// Process tool result content: sync fast path, then async LLM summarization.
///
/// Returns the processed content string.
/// - Short content: returned as-is (sync)
/// - Medium content: truncated (sync)
/// - Very long content: LLM summarized (async) with sync fallback on error
async fn process_result_content(
    client: &LlmClient,
    model: &str,
    content: &str,
) -> String {
    // Try sync fast path first
    match tools::process_tool_result_content(content) {
        Some(processed) => processed,
        None => {
            // Content exceeds summarize threshold — use LLM summarization
            tracing::info!(
                "Tool result {} chars exceeds summarize threshold, using LLM summarization",
                content.len()
            );
            let summary = long_text::summarize_long_content(client, model, content).await;
            if summary.is_empty() {
                // Fallback to sync head+tail truncation
                tools::process_tool_result_content_fallback(content)
            } else {
                summary
            }
        }
    }
}

/// Inject progressive disclosure docs for skill tools being called for the first time.
/// Returns `true` if docs were injected (caller should re-prompt LLM).
///
/// IMPORTANT: When this returns `true`, the caller must NOT have an assistant message
/// with `tool_calls` pending in `messages` without corresponding tool results.
/// The OpenAI API requires every tool_call to have a matching tool result message.
///
/// This function handles it by:
/// 1. Removing the last assistant message (which contains the tool_calls)
/// 2. Injecting the docs as a user message (not system, to avoid breaking the flow)
fn inject_progressive_disclosure(
    tool_calls: &[ToolCall],
    skills: &[LoadedSkill],
    documented_skills: &mut HashSet<String>,
    messages: &mut Vec<ChatMessage>,
) -> bool {
    let mut new_docs = Vec::new();

    for tc in tool_calls {
        let tool_name = &tc.function.name;
        if !tools::is_builtin_tool(tool_name) && !documented_skills.contains(tool_name) {
            if let Some(skill) = skills::find_skill_by_tool_name(skills, tool_name) {
                if let Some(docs) = prompt::get_skill_full_docs(skill) {
                    new_docs.push((tool_name.clone(), docs));
                    documented_skills.insert(tool_name.clone());
                }
            }
        }
    }

    if new_docs.is_empty() {
        return false;
    }

    // Remove the assistant message with tool_calls that was just added.
    // The API requires tool_calls to be followed by tool result messages,
    // but we're going to re-prompt instead of executing.
    if let Some(last) = messages.last() {
        if last.role == "assistant" && last.tool_calls.is_some() {
            messages.pop();
        }
    }

    // Inject documentation as a user message prompting re-call
    let docs_text: Vec<String> = new_docs
        .iter()
        .map(|(name, docs)| {
            format!(
                "## Full Documentation for skill: {}\n\n{}\n",
                name, docs
            )
        })
        .collect();

    let tool_names: Vec<&str> = new_docs.iter().map(|(n, _)| n.as_str()).collect();
    messages.push(ChatMessage::user(&format!(
        "Before calling {}, here is the full documentation you need:\n\n{}\n\
         Please now call the skill with the correct parameters based on the documentation above.",
        tool_names.join(", "),
        docs_text.join("\n")
    )));

    true
}

/// Build the final `AgentResult` from the message history.
fn build_agent_result(
    messages: Vec<ChatMessage>,
    tool_calls_count: usize,
    iterations: usize,
) -> AgentResult {
    let final_response = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && m.content.is_some())
        .and_then(|m| m.content.clone())
        .unwrap_or_else(|| "[Agent completed without text response]".to_string());

    AgentResult {
        response: final_response,
        messages,
        tool_calls_count,
        iterations,
    }
}
