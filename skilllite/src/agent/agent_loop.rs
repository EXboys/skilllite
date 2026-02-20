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
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

use super::extensions::{self, MemoryVectorContext};
use super::llm::{self, LlmClient};
use crate::config::EmbeddingConfig;
use super::long_text;
use super::prompt;
use super::skills::{self, LoadedSkill};
use super::task_planner::TaskPlanner;
use super::types::*;

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
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty())
        .then(|| MemoryVectorContext {
            client: &client,
            embed_config: &embed_config,
        });

    let registry = extensions::ExtensionRegistry::new(
        config.enable_memory,
        config.enable_memory_vector,
        skills,
    );
    let all_tools = registry.all_tool_definitions();

    // Build system prompt
    let system_prompt = prompt::build_system_prompt(
        config.system_prompt.as_deref(),
        skills,
        &config.workspace,
        session_key,
        config.enable_memory,
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
    let mut context_overflow_retries = 0usize;

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
            Ok(resp) => {
                context_overflow_retries = 0;
                resp
            }
            Err(e) => {
                let err_msg = e.to_string();
                if llm::is_context_overflow_error(&err_msg) {
                    context_overflow_retries += 1;
                    if context_overflow_retries >= MAX_CONTEXT_OVERFLOW_RETRIES {
                        tracing::error!(
                            "Context overflow persists after {} retries, giving up",
                            MAX_CONTEXT_OVERFLOW_RETRIES
                        );
                        return Err(e);
                    }
                    let recovery_chars = get_tool_result_recovery_max_chars();
                    tracing::warn!(
                        "Context overflow (attempt {}/{}), truncating tool messages to {} chars",
                        context_overflow_retries, MAX_CONTEXT_OVERFLOW_RETRIES, recovery_chars
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
        if let Some(tool_calls) = tool_calls {
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

                let mut result = execute_tool_call(
                    &registry,
                    tool_name,
                    arguments,
                    workspace,
                    event_sink,
                    embed_ctx.as_ref(),
                )
                .await;
                result.tool_call_id = tc.id.clone();

                // Process long content (sync fast path → async LLM summarization fallback)
                result.content =
                    process_result_content(&client, &config.model, tool_name, &result.content).await;

                event_sink.on_tool_result(tool_name, &result.content, result.is_error);
                messages.push(ChatMessage::tool_result(&result.tool_call_id, &result.content));

                total_tool_calls += 1;
            }
        }

        // Check tool call depth limit
        if total_tool_calls >= config.max_iterations * config.max_tool_calls_per_task {
            tracing::warn!("Agent loop reached total tool call limit");
            break;
        }
    }

    Ok(build_agent_result(messages, total_tool_calls, iterations, Vec::new()))
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
    session_key: Option<&str>,
) -> Result<AgentResult> {
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let workspace = Path::new(&config.workspace);
    let embed_config = EmbeddingConfig::from_env();
    let embed_ctx = (config.enable_memory_vector && !config.api_key.is_empty())
        .then(|| MemoryVectorContext {
            client: &client,
            embed_config: &embed_config,
        });

    let registry = extensions::ExtensionRegistry::new(
        config.enable_memory,
        config.enable_memory_vector,
        skills,
    );
    let all_tools = registry.all_tool_definitions();

    // ── Task planning ──────────────────────────────────────────────────────
    let mut planner = TaskPlanner::new(Some(workspace));

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

    // ── Unified path: ALWAYS run with tools ─────────────────────────────
    // No "empty plan = no tools" branch. Planner may return [] for simple
    // queries; we still pass tools so LLM can use them if needed.
    // When planner is empty: use standard prompt (no task list). When non-empty: use task-aware prompt.

    let system_prompt = if planner.is_empty() {
        prompt::build_system_prompt(
            config.system_prompt.as_deref(),
            skills,
            &config.workspace,
            session_key,
            config.enable_memory,
        )
    } else {
        let mut prompt = planner.build_task_system_prompt(skills);
        if let Some(sk) = session_key {
            prompt.push_str(&format!(
                "\n\nCurrent session: {} — use session_key '{}' for chat_history and chat_plan.\n\
                 /compact compresses conversation; result appears as [compaction] in chat_history. \
                 When user asks about 最新的/compact or /compact效果, read chat_history with session_key '{}'.",
                sk, sk, sk
            ));
        }
        prompt
    };

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
    let mut context_overflow_retries = 0usize;

    // Plan-based budget: effective_max = min(global, num_tasks * per_task)
    // When num_tasks=0 (planner returned empty), use max_iterations so we still run the loop
    let num_tasks = planner.task_list.len();
    let effective_max = if num_tasks == 0 {
        config.max_iterations
    } else {
        config
            .max_iterations
            .min(num_tasks * config.max_tool_calls_per_task)
    };

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

        // ── Suppress streaming for hallucination-prone iterations ────
        // When no tools have been called yet and the plan requires tool
        // execution, the LLM will very likely hallucinate a "completed"
        // summary.  Use non-streaming call so the user never sees it.
        let suppress_stream = total_tool_calls == 0 && {
            planner.task_list.iter().any(|t| {
                !t.completed
                    && t.tool_hint
                        .as_ref()
                        .map_or(false, |h| h != "analysis")
            })
        };

        // Call LLM (with shared context overflow recovery for both paths)
        let llm_result = if suppress_stream {
            client
                .chat_completion(&config.model, &messages, tools_ref, config.temperature)
                .await
        } else {
            client
                .chat_completion_stream(
                    &config.model, &messages, tools_ref, config.temperature, event_sink,
                )
                .await
        };

        let response = match llm_result {
            Ok(resp) => {
                context_overflow_retries = 0;
                resp
            }
            Err(e) => {
                let err_msg = e.to_string();
                if llm::is_context_overflow_error(&err_msg) {
                    context_overflow_retries += 1;
                    if context_overflow_retries >= MAX_CONTEXT_OVERFLOW_RETRIES {
                        tracing::error!(
                            "Context overflow persists after {} retries, giving up",
                            MAX_CONTEXT_OVERFLOW_RETRIES
                        );
                        return Err(e);
                    }
                    let recovery_chars = get_tool_result_recovery_max_chars();
                    tracing::warn!(
                        "Context overflow (attempt {}/{}), truncating tool messages to {} chars",
                        context_overflow_retries, MAX_CONTEXT_OVERFLOW_RETRIES, recovery_chars
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

        let has_tool_calls = tool_calls
            .as_ref()
            .map_or(false, |tc| !tc.is_empty());

        // If streaming was suppressed but the LLM did return tool calls
        // (not a hallucination), emit the assistant text now so the user
        // can see it.
        if suppress_stream && has_tool_calls {
            if let Some(ref content) = assistant_content {
                event_sink.on_text(content);
            }
        }

        if !has_tool_calls {
            // ── Hallucination guard: no tools executed yet ───────────────
            // If no tool calls have been made AT ALL and the plan contains
            // pending tasks that require tool execution, reject silently.
            // Since we used non-streaming above, the user saw nothing.
            if suppress_stream {
                // Pop the hallucinated assistant message from history
                if let Some(last) = messages.last() {
                    if last.role == "assistant" {
                        messages.pop();
                    }
                }
                tracing::info!(
                    "Anti-hallucination: silently rejected text-only response \
                     (no tools executed yet, plan requires tool tasks)"
                );
                consecutive_no_tool += 1;

                if consecutive_no_tool >= max_no_tool_retries {
                    tracing::warn!(
                        "LLM failed to start execution after {} attempts, stopping",
                        max_no_tool_retries
                    );
                    break;
                }

                // Send a strong nudge forcing actual execution
                if let Some(nudge) = planner.build_nudge_message() {
                    messages.push(ChatMessage::user(&format!(
                        "CRITICAL: You just described what you would do but did NOT \
                         actually execute anything. The task plan has been generated — \
                         now you must EXECUTE each task step by step. Call the required \
                         tools NOW.\n\n{}",
                        nudge
                    )));
                    continue;
                }
                break;
            }

            // ── Normal completion check (post first iteration) ──────────
            // LLM has been through at least one work iteration — accept
            // text-based "Task X completed" claims, but validate each one.
            let mut made_progress = false;
            if let Some(ref content) = assistant_content {
                let completed_ids = planner.check_completion_in_content(content);
                // Snapshot the counter BEFORE processing completions so that
                // ALL tasks in this batch benefit from tool calls made in
                // previous iterations. The old code reset the counter after
                // each task, which caused Tasks 2, 3, … in the same response
                // to be falsely rejected.
                let had_tool_calls_for_batch = tool_calls_current_task > 0;
                // Fallback: if we've executed any tools in this session, trust completion
                // claims for tool-requiring tasks (avoids false rejection when counter
                // was reset but tools were run in a prior iteration).
                let session_had_tools = total_tool_calls > 0;
                for completed_id in completed_ids {
                    // Anti-hallucination: reject completion claims for tasks
                    // that require tool execution when no tool calls were made
                    // since the last batch of completions.
                    let task_needs_tool = planner.task_list.iter()
                        .find(|t| t.id == completed_id)
                        .map_or(false, |t| {
                            t.tool_hint.as_ref().map_or(false, |h| h != "analysis")
                        });
                    if task_needs_tool && !had_tool_calls_for_batch && !session_had_tools {
                        tracing::info!(
                            "Anti-hallucination: rejected text-only completion for task {} \
                             (requires tool but no tool calls made since last completion batch)",
                            completed_id
                        );
                        continue;
                    }
                    planner.mark_completed(completed_id);
                    event_sink.on_task_progress(completed_id, true);
                    made_progress = true;
                }
                // Reset counter AFTER the entire batch so the NEXT iteration
                // also needs its own tool calls before tasks can be completed.
                if made_progress {
                    tool_calls_current_task = 0;
                    consecutive_no_tool = 0;
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

            // If progress was made (some tasks completed), skip the nudge
            if made_progress {
                continue;
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

            // No nudge available, emit and break
            if let Some(ref content) = assistant_content {
                event_sink.on_text(content);
            }
            break;
        }

        // ── Process tool calls ─────────────────────────────────────────────
        consecutive_no_tool = 0;
        let tool_calls = match tool_calls {
            Some(tc) => tc,
            None => continue,
        };

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

            let mut result = if tool_name.as_str() == "update_task_plan" {
                handle_update_task_plan(arguments, &mut planner, event_sink)
            } else {
                execute_tool_call(
                    &registry,
                    tool_name,
                    arguments,
                    workspace,
                    event_sink,
                    embed_ctx.as_ref(),
                )
                .await
            };
            result.tool_call_id = tc.id.clone();

            result.content =
                process_result_content(&client, &config.model, tool_name, &result.content).await;

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
        // After tool execution, allow completing consecutive tasks when the
        // LLM called tools for multiple tasks in a single batch.
        // Update current_task_id after each completion so the next task
        // in sequence can also be completed.
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
                // Update current_task_id so the next consecutive task
                // can also be completed in this batch.
                current_task_id = planner.current_task().map(|t| t.id);
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
            let focus_msg = if tool_hint == "file_operation" {
                format!(
                    "Task progress update:\n{}\n\n\
                     Current task to execute: Task {} - {}\n\n\
                     ⚡ Use `write_output` or `preview_server` NOW. \
                     ⛔ Do NOT call any skill tools.",
                    task_list_json, current.id, current.description
                )
            } else if !tool_hint.is_empty() && tool_hint != "analysis" {
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

    Ok(build_agent_result(messages, total_tool_calls, iterations, planner.task_list.clone()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Shared helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle update_task_plan: parse new tasks, replace planner.task_list, notify event_sink.
fn handle_update_task_plan(
    arguments: &str,
    planner: &mut TaskPlanner,
    event_sink: &mut dyn EventSink,
) -> super::types::ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "update_task_plan".to_string(),
                content: format!("Invalid JSON: {}", e),
                is_error: true,
            };
        }
    };
    let tasks_arr = match args.get("tasks").and_then(|t| t.as_array()) {
        Some(a) => a.clone(),
        None => {
            return super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "update_task_plan".to_string(),
                content: "Missing or invalid 'tasks' array".to_string(),
                is_error: true,
            };
        }
    };
    let mut new_tasks = Vec::new();
    for (i, t) in tasks_arr.iter().enumerate() {
        let id = t.get("id").and_then(|v| v.as_u64()).unwrap_or((i + 1) as u64) as u32;
        let description = t
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tool_hint = t.get("tool_hint").and_then(|v| v.as_str()).map(String::from);
        let completed = t.get("completed").and_then(|v| v.as_bool()).unwrap_or(false);
        new_tasks.push(Task {
            id,
            description,
            tool_hint,
            completed,
        });
    }
    if new_tasks.is_empty() {
        return super::types::ToolResult {
            tool_call_id: String::new(),
            tool_name: "update_task_plan".to_string(),
            content: "Task list cannot be empty".to_string(),
            is_error: true,
        };
    }
    planner.task_list = new_tasks.clone();
    event_sink.on_task_plan(&planner.task_list);
    let reason = args.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    let mut content = format!(
        "Task plan updated ({} tasks). Continue with the new plan.",
        new_tasks.len()
    );
    if !reason.is_empty() {
        content.push_str(&format!("\nReason: {}", reason));
    }
    super::types::ToolResult {
        tool_call_id: String::new(),
        tool_name: "update_task_plan".to_string(),
        content,
        is_error: false,
    }
}

/// Execute a single tool call via ExtensionRegistry.
async fn execute_tool_call(
    registry: &extensions::ExtensionRegistry<'_>,
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
    embed_ctx: Option<&extensions::MemoryVectorContext<'_>>,
) -> ToolResult {
    registry
        .execute(tool_name, arguments, workspace, event_sink, embed_ctx)
        .await
}

/// Tools whose results must never be LLM-summarized because the LLM needs the
/// content verbatim (e.g. for content transfer between files, or re-use).
/// For these tools, we only do simple truncation as a last resort.
const CONTENT_PRESERVING_TOOLS: &[&str] = &["read_file", "chat_history"];

/// Process tool result content: sync fast path, then async LLM summarization.
///
/// Returns the processed content string.
/// - Short content: returned as-is (sync)
/// - Medium content: truncated (sync)
/// - Very long content: LLM summarized (async) with sync fallback on error
///
/// `tool_name` controls whether LLM summarization is allowed. For content-
/// preserving tools like `read_file`, only simple truncation is used so the
/// actual content is never destroyed by summarization.
async fn process_result_content(
    client: &LlmClient,
    model: &str,
    tool_name: &str,
    content: &str,
) -> String {
    // Try sync fast path first
    match extensions::process_tool_result_content(content) {
        Some(processed) => processed,
        None => {
            // Content exceeds summarize threshold.
            // For content-preserving tools (read_file), never summarize — the
            // LLM needs the actual content. Use head+tail truncation instead.
            if CONTENT_PRESERVING_TOOLS.contains(&tool_name) {
                tracing::info!(
                    "Tool '{}' result {} chars exceeds threshold, using head+tail truncation (no LLM summarization)",
                    tool_name, content.len()
                );
                extensions::process_tool_result_content_fallback(content)
            } else {
                tracing::info!(
                    "Tool '{}' result {} chars exceeds summarize threshold, using LLM summarization",
                    tool_name, content.len()
                );
                let summary = long_text::summarize_long_content(client, model, content).await;
                if summary.is_empty() {
                    // Fallback to sync head+tail truncation
                    extensions::process_tool_result_content_fallback(content)
                } else {
                    summary
                }
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
        // Normalize tool name for dedup (frontend-design == frontend_design)
        let normalized = tool_name.replace('-', "_").to_lowercase();
        if !extensions::is_builtin_tool(tool_name) && !documented_skills.contains(&normalized) {
            // Try by tool definition first, then by skill name (for reference-only skills)
            let skill = skills::find_skill_by_tool_name(skills, tool_name)
                .or_else(|| skills::find_skill_by_name(skills, tool_name));
            if let Some(skill) = skill {
                if let Some(docs) = prompt::get_skill_full_docs(skill) {
                    new_docs.push((tool_name.clone(), docs));
                    documented_skills.insert(normalized);
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
    task_plan: Vec<Task>,
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
        task_plan,
    }
}
