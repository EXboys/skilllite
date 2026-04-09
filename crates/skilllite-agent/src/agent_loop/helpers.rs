//! Shared helpers for the agent loop: tool execution, result processing,
//! progressive disclosure, task plan handling, and result building.

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use crate::Result;

use super::super::extensions::{self};
use super::super::goal_boundaries::{self, GoalBoundaries};
use super::super::goal_contract::{self, GoalContract, RiskLevel};
use super::super::llm::LlmClient;
use super::super::long_text;
use super::super::prompt;
use super::super::skills::{self, LoadedSkill};
use super::super::task_planner::TaskPlanner;
use super::super::types::{safe_truncate, *};

/// Unwrap double-encoded JSON: some models (especially DeepSeek in long contexts)
/// wrap arguments in extra quotes, producing `Value::String` instead of `Value::Object`.
/// This helper detects that case and re-parses the inner string.
fn unwrap_double_encoded_json(v: Value) -> Value {
    if let Some(s) = v.as_str() {
        if let Ok(inner) = serde_json::from_str::<Value>(s) {
            if inner.is_object() || inner.is_array() {
                tracing::warn!(
                    "Unwrapped double-encoded JSON arguments (outer was a string containing {})",
                    if inner.is_object() { "object" } else { "array" }
                );
                return inner;
            }
        }
    }
    v
}

/// Handle update_task_plan: parse new tasks, sanitize & enhance (same as initial planning),
/// replace planner.task_list, notify event_sink.
pub(super) fn handle_update_task_plan(
    arguments: &str,
    planner: &mut TaskPlanner,
    skills: &[LoadedSkill],
    event_sink: &mut dyn EventSink,
) -> super::super::types::ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => unwrap_double_encoded_json(v),
        Err(e) => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "update_task_plan".to_string(),
                content: format!("Invalid JSON: {}", e),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };
    let tasks_arr = match args.get("tasks") {
        Some(v) => {
            if let Some(a) = v.as_array() {
                a.clone()
            } else if let Some(s) = v.as_str() {
                match serde_json::from_str::<Vec<Value>>(s) {
                    Ok(a) => a,
                    Err(_) => {
                        return super::super::types::ToolResult {
                            tool_call_id: String::new(),
                            tool_name: "update_task_plan".to_string(),
                            content: format!(
                                "'tasks' must be a JSON array, got a string that is not valid JSON array. \
                                 Pass tasks as a real array: {{\"tasks\": [{{...}}]}} not {{\"tasks\": \"[...]\"}}. \
                                 Received string preview: {:?}",
                                &s[..s.len().min(120)]
                            ),
                            is_error: true,
                            counts_as_failure: true,
                        };
                    }
                }
            } else {
                return super::super::types::ToolResult {
                    tool_call_id: String::new(),
                    tool_name: "update_task_plan".to_string(),
                    content: format!(
                        "'tasks' must be a JSON array, got unexpected type: {}. \
                         Pass tasks as: {{\"tasks\": [{{\"id\": 1, \"description\": \"...\"}}]}}.",
                        v
                    ),
                    is_error: true,
                    counts_as_failure: true,
                };
            }
        }
        None => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "update_task_plan".to_string(),
                content: "Missing required field: 'tasks'. Pass an array of task objects."
                    .to_string(),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };
    let mut new_tasks = Vec::new();
    for (i, t) in tasks_arr.iter().enumerate() {
        let id = t
            .get("id")
            .and_then(|v| v.as_u64())
            .unwrap_or((i + 1) as u64) as u32;
        let description = t
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tool_hint = t
            .get("tool_hint")
            .and_then(|v| v.as_str())
            .map(String::from);
        let completed = t
            .get("completed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        new_tasks.push(Task {
            id,
            description,
            tool_hint,
            completed,
        });
    }
    if new_tasks.is_empty() {
        return super::super::types::ToolResult {
            tool_call_id: String::new(),
            tool_name: "update_task_plan".to_string(),
            content: "Task list cannot be empty".to_string(),
            is_error: true,
            counts_as_failure: true,
        };
    }
    // Apply same sanitize & enhance as initial planning (strip unavailable tool_hints, add SKILL.md if needed).
    planner.sanitize_and_enhance_tasks(&mut new_tasks, skills);

    // Preserve completed tasks — only replace pending portion of the plan.
    let completed_tasks: Vec<Task> = planner
        .task_list
        .iter()
        .filter(|t| t.completed)
        .cloned()
        .collect();
    let next_id = completed_tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    for (i, t) in new_tasks.iter_mut().enumerate() {
        t.id = next_id + i as u32;
        t.completed = false;
    }
    let new_count = new_tasks.len();
    let mut merged = completed_tasks;
    merged.extend(new_tasks);
    planner.task_list = merged;
    event_sink.on_task_plan(&planner.task_list);
    let reason = args.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    let mut content = format!(
        "Task plan updated ({} tasks). Continue with the new plan.",
        new_count
    );
    if !reason.is_empty() {
        content.push_str(&format!("\nReason: {}", reason));
    }
    super::super::types::ToolResult {
        tool_call_id: String::new(),
        tool_name: "update_task_plan".to_string(),
        content,
        is_error: false,
        counts_as_failure: false,
    }
}

/// Handle complete_task: validate task_id matches current task, then mark it done.
///
/// This is the structured completion signal that replaces text-based "Task X completed"
/// pattern matching. Only the *current* (first uncompleted) task may be completed.
pub(super) fn handle_complete_task(
    arguments: &str,
    planner: &mut TaskPlanner,
    event_sink: &mut dyn EventSink,
) -> super::super::types::ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => unwrap_double_encoded_json(v),
        Err(e) => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "complete_task".to_string(),
                content: format!("Invalid JSON: {}", e),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };

    let task_id = match args.get("task_id") {
        Some(v) => {
            if let Some(n) = v.as_u64() {
                n as u32
            } else if let Some(s) = v.as_str() {
                match s.parse::<u64>() {
                    Ok(n) => n as u32,
                    Err(_) => {
                        return super::super::types::ToolResult {
                            tool_call_id: String::new(),
                            tool_name: "complete_task".to_string(),
                            content: format!(
                                "task_id must be an integer, got string {:?} which is not a valid number. \
                                 Pass task_id as a bare number, e.g. {{\"task_id\": 1}} not {{\"task_id\": \"abc\"}}.",
                                s
                            ),
                            is_error: true,
                            counts_as_failure: true,
                        };
                    }
                }
            } else {
                return super::super::types::ToolResult {
                    tool_call_id: String::new(),
                    tool_name: "complete_task".to_string(),
                    content: format!(
                        "task_id must be an integer, got unexpected type: {}. \
                         Pass task_id as a bare number, e.g. {{\"task_id\": 1}}.",
                        v
                    ),
                    is_error: true,
                    counts_as_failure: true,
                };
            }
        }
        None => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "complete_task".to_string(),
                content:
                    "Missing required field: task_id. Pass the integer id of the completed task."
                        .to_string(),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };

    let current_id = planner.current_task().map(|t| t.id);
    if Some(task_id) != current_id {
        let msg = match current_id {
            Some(cid) => format!(
                "Cannot complete task {} — current task is {}. Complete tasks in order.",
                task_id, cid
            ),
            None => "All tasks are already completed.".to_string(),
        };
        return super::super::types::ToolResult {
            tool_call_id: String::new(),
            tool_name: "complete_task".to_string(),
            content: msg,
            is_error: true,
            counts_as_failure: true,
        };
    }

    let summary = args
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let completion_type = match args
        .get("completion_type")
        .and_then(|v| v.as_str())
        .map(str::trim)
    {
        None | Some("") => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "complete_task".to_string(),
                content:
                    "Missing required field: completion_type. Use one of success, partial_success, failure."
                        .to_string(),
                is_error: true,
                counts_as_failure: true,
            };
        }
        Some("success") => TaskCompletionType::Success,
        Some("partial_success") => TaskCompletionType::PartialSuccess,
        Some("failure") => TaskCompletionType::Failure,
        Some(other) => {
            return super::super::types::ToolResult {
                tool_call_id: String::new(),
                tool_name: "complete_task".to_string(),
                content: format!(
                    "Invalid completion_type: {:?}. Allowed values: success, partial_success, failure.",
                    other
                ),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };

    planner.mark_completed(task_id);
    event_sink.on_task_progress(task_id, true, &planner.task_list);
    tracing::info!(
        "complete_task: task {} marked done. completion_type={}. summary={:?}",
        task_id,
        completion_type.as_str(),
        summary
    );

    super::super::types::ToolResult {
        tool_call_id: String::new(),
        tool_name: "complete_task".to_string(),
        content: format!(
            r#"{{"success": true, "task_id": {}, "completion_type": "{}", "message": "Task {} marked as completed"}}"#,
            task_id,
            completion_type.as_str(),
            task_id
        ),
        is_error: false,
        counts_as_failure: false,
    }
}

/// Execute a single tool call via ExtensionRegistry.
/// `planning_ctx` is required for PlanningControl tools (complete_task, update_task_plan).
pub(super) async fn execute_tool_call(
    registry: &extensions::ExtensionRegistry<'_>,
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
    embed_ctx: Option<&extensions::MemoryVectorContext<'_>>,
    planning_ctx: Option<&mut dyn extensions::PlanningControlExecutor>,
) -> ToolResult {
    registry
        .execute(
            tool_name,
            arguments,
            workspace,
            event_sink,
            embed_ctx,
            planning_ctx,
        )
        .await
}

/// Tools whose results must never be LLM-summarized because the LLM needs the
/// content verbatim (e.g. for content transfer between files, or re-use).
/// For these tools, we only do simple truncation as a last resort.
pub(super) const CONTENT_PRESERVING_TOOLS: &[&str] = &["read_file", "chat_history"];

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
pub(super) async fn process_result_content(
    client: &LlmClient,
    model: &str,
    tool_name: &str,
    content: &str,
) -> String {
    if tool_name == "read_file" {
        return extensions::process_read_file_tool_result_content(content);
    }

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
pub(super) fn inject_progressive_disclosure(
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
        if !documented_skills.contains(&normalized) {
            // Try by tool definition first, then by skill name (for reference-only skills).
            // This keeps progressive disclosure aligned with the actual skill registry
            // instead of maintaining a parallel built-in allowlist.
            if let Some(skill) = skills::find_skill_by_tool_name(skills, tool_name)
                .or_else(|| skills::find_skill_by_name(skills, tool_name))
            {
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
        .map(|(name, docs)| format!("## Full Documentation for skill: {}\n\n{}\n", name, docs))
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

/// Extract goal boundaries via LLM (fallback when regex returns empty).
/// Enabled by SKILLLITE_GOAL_LLM_EXTRACT=1.
pub(super) async fn extract_goal_boundaries_llm(
    client: &LlmClient,
    model: &str,
    goal: &str,
    usage_totals: Option<&mut LlmUsageTotals>,
) -> Result<GoalBoundaries> {
    const PROMPT: &str = r#"Extract goal boundaries from the user's goal. Return JSON only:
{"scope": "...", "exclusions": "...", "completion_conditions": "..."}
- scope: what is in scope for this goal (optional, null if unclear)
- exclusions: what to avoid or exclude (optional, null if unclear)
- completion_conditions: when the task is considered done (optional, null if unclear)
Use null for any field you cannot infer. Output only valid JSON, no markdown, no other text."#;

    let messages = vec![ChatMessage::system(PROMPT), ChatMessage::user(goal)];

    let resp = client
        .chat_completion(model, &messages, None, Some(0.2), usage_totals)
        .await?;

    let raw = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    let raw = raw.trim();
    let json_str = if raw.starts_with("```json") {
        raw.trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else if raw.starts_with("```") {
        raw.trim_start_matches("```").trim_end_matches("```").trim()
    } else {
        raw
    };

    let v: Value = serde_json::from_str(json_str).map_err(|e| {
        crate::Error::validation(format!("Goal boundaries JSON parse error: {}", e))
    })?;

    let scope = v
        .get("scope")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let exclusions = v
        .get("exclusions")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let completion_conditions = v
        .get("completion_conditions")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(GoalBoundaries {
        scope,
        exclusions,
        completion_conditions,
    })
}

fn strip_json_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.starts_with("```json") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    }
}

fn parse_goal_contract_json(json_str: &str) -> Result<GoalContract> {
    let v: Value = serde_json::from_str(json_str)
        .map_err(|e| crate::Error::validation(format!("Goal contract JSON parse error: {}", e)))?;

    let goal = v
        .get("goal")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let acceptance = v
        .get("acceptance")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let constraints = v
        .get("constraints")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let deadline = v
        .get("deadline")
        .and_then(|s| s.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let risk_level = v
        .get("risk_level")
        .and_then(|s| s.as_str())
        .and_then(RiskLevel::from_text);

    Ok(GoalContract {
        goal,
        acceptance,
        constraints,
        deadline,
        risk_level,
    })
}

/// Extract goal contract via LLM for better robustness on free-form input.
pub(super) async fn extract_goal_contract_llm(
    client: &LlmClient,
    model: &str,
    goal: &str,
    usage_totals: Option<&mut LlmUsageTotals>,
) -> Result<GoalContract> {
    const PROMPT: &str = r#"Extract an executable goal contract from user goal text. Return JSON only:
{"goal":"...","acceptance":"...","constraints":"...","deadline":"...","risk_level":"low|medium|high|critical"}
- Use null for unknown fields
- risk_level must be one of low|medium|high|critical or null
- Output only valid JSON; no markdown or explanation."#;

    let messages = vec![ChatMessage::system(PROMPT), ChatMessage::user(goal)];
    let resp = client
        .chat_completion(model, &messages, None, Some(0.2), usage_totals)
        .await?;

    let raw = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    parse_goal_contract_json(strip_json_code_fence(&raw))
}

/// Hybrid extraction for goal contract:
/// - Try LLM first (richer semantic parsing for free-form goals)
/// - Fallback to regex-based extractor on any failure
pub(super) async fn extract_goal_contract_hybrid(
    client: &LlmClient,
    model: &str,
    goal: &str,
    usage_totals: Option<&mut LlmUsageTotals>,
) -> GoalContract {
    match extract_goal_contract_llm(client, model, goal, usage_totals).await {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            tracing::info!("Goal contract LLM extraction empty, fallback to regex extraction");
            goal_contract::extract_goal_contract(goal)
        }
        Err(e) => {
            tracing::warn!(
                "Goal contract LLM extraction failed: {}, fallback to regex extraction",
                e
            );
            goal_contract::extract_goal_contract(goal)
        }
    }
}

/// Hybrid extraction: regex first, LLM fallback when regex returns empty.
/// LLM fallback only when SKILLLITE_GOAL_LLM_EXTRACT=1.
pub(super) async fn extract_goal_boundaries_hybrid(
    client: &LlmClient,
    model: &str,
    goal: &str,
    usage_totals: Option<&mut LlmUsageTotals>,
) -> Result<GoalBoundaries> {
    let regex_result = goal_boundaries::extract_goal_boundaries(goal);
    if !regex_result.is_empty() {
        return Ok(regex_result);
    }
    if std::env::var("SKILLLITE_GOAL_LLM_EXTRACT").as_deref() == Ok("1") {
        tracing::info!("Goal boundaries regex empty, trying LLM extraction");
        extract_goal_boundaries_llm(client, model, goal, usage_totals).await
    } else {
        Ok(regex_result)
    }
}

/// `complete_task` returns a JSON ack (often >80 chars). It must not be used as the user-facing
/// tool summary — the real answer is in earlier tool rows (e.g. `weather`).
fn is_complete_task_tool_result_body(s: &str) -> bool {
    let t = s.trim();
    if !t.starts_with('{') {
        return false;
    }
    let Ok(v) = serde_json::from_str::<Value>(t) else {
        return false;
    };
    let Some(obj) = v.as_object() else {
        return false;
    };
    obj.contains_key("task_id")
        && obj.contains_key("completion_type")
        && obj.get("success").and_then(|x| x.as_bool()).is_some()
}

fn tool_body_usable_for_user_summary(s: &str) -> bool {
    !s.trim().is_empty() && !is_complete_task_tool_result_body(s)
}

/// True when the message list ends with one or more `role == "tool"` rows and at least one
/// of those trailing tool results has at least `min_chars` non-whitespace characters.
///
/// Used after a planning-mode tool batch: the model's assistant prose alongside tool calls is
/// often suppressed, so `assistant_content` is empty even though the user already has a rich
/// answer in tool output (e.g. weather JSON). Without this check we would almost always schedule
/// `pending_closing_user_reply`, causing an extra LLM turn and a duplicate assistant bubble.
///
/// Bodies that look like `complete_task` results are ignored — they are not user answers.
pub(super) fn latest_trailing_tool_results_include_substantive_output(
    messages: &[ChatMessage],
    min_chars: usize,
) -> bool {
    for m in messages.iter().rev() {
        if m.role != "tool" {
            break;
        }
        if let Some(c) = m.content.as_ref() {
            if tool_body_usable_for_user_summary(c) && c.trim().len() >= min_chars {
                return true;
            }
        }
    }
    false
}

const TOOL_FALLBACK_PICK_MIN_CHARS: usize = 80;
/// Cap for tool-derived user-visible summaries (RPC + transcript).
pub(super) const TRAILING_TOOL_SUMMARY_MAX_BYTES: usize = 12 * 1024;

/// Picks one trailing `role=tool` body for user-visible summary: prefer the first (from the end)
/// with at least `min_chars` non-whitespace characters, else the first non-empty trailing tool.
/// Returns `None` if there is no qualifying tool message.
pub(super) fn pick_substantive_trailing_tool_content(
    messages: &[ChatMessage],
    min_chars: usize,
    max_bytes: usize,
) -> Option<String> {
    let mut trailing_tool_texts: Vec<&str> = Vec::new();
    for m in messages.iter().rev() {
        if m.role != "tool" {
            break;
        }
        if let Some(c) = m.content.as_ref() {
            let t = c.trim();
            if tool_body_usable_for_user_summary(t) {
                trailing_tool_texts.push(t);
            }
        }
    }
    let picked = trailing_tool_texts
        .iter()
        .copied()
        .find(|t| t.len() >= min_chars)
        .or_else(|| trailing_tool_texts.first().copied())?;
    Some(safe_truncate(picked, max_bytes).to_string())
}

fn final_response_from_assistant_or_tools(messages: &[ChatMessage]) -> String {
    if let Some(s) = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(|m| m.content.as_ref())
        .map(|c| c.trim())
        .filter(|t| !t.is_empty())
    {
        return s.to_string();
    }

    if let Some(s) = pick_substantive_trailing_tool_content(
        messages,
        TOOL_FALLBACK_PICK_MIN_CHARS,
        TRAILING_TOOL_SUMMARY_MAX_BYTES,
    ) {
        return s;
    }

    "[Agent completed without text response]".to_string()
}

/// Build the final `AgentResult` from the message history.
pub(super) fn build_agent_result(
    messages: Vec<ChatMessage>,
    tool_calls_count: usize,
    iterations: usize,
    task_plan: Vec<Task>,
    feedback: ExecutionFeedback,
) -> AgentResult {
    let final_response = if !feedback.task_completed && !task_plan.is_empty() {
        let completed = task_plan.iter().filter(|t| t.completed).count();
        let total = task_plan.len();
        format!(
            "任务未完成（{}/{}）。当前仅获得部分信息，未满足完整目标。请继续执行任务，或重规划后以 `complete_task(..., completion_type=\"partial_success|failure\")` 结束本轮。",
            completed, total
        )
    } else {
        final_response_from_assistant_or_tools(&messages)
    };

    AgentResult {
        response: final_response,
        messages,
        tool_calls_count,
        iterations,
        task_plan,
        feedback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_planner::TaskPlanner;
    use crate::types::{ChatMessage, ExecutionFeedback, SilentEventSink, Task};

    #[test]
    fn handle_update_task_plan_rejects_invalid_json() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let r = handle_update_task_plan("not json", &mut planner, &[], &mut sink);
        assert!(r.is_error);
        assert!(r.content.contains("Invalid JSON"));
    }

    #[test]
    fn handle_update_task_plan_requires_tasks_array() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let r = handle_update_task_plan(r#"{"reason":"x"}"#, &mut planner, &[], &mut sink);
        assert!(r.is_error);
        assert!(r.content.contains("tasks"));
    }

    #[test]
    fn handle_update_task_plan_rejects_empty_task_list() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let r = handle_update_task_plan(r#"{"tasks":[]}"#, &mut planner, &[], &mut sink);
        assert!(r.is_error);
        assert!(r.content.contains("empty"));
    }

    #[test]
    fn handle_update_task_plan_merges_with_completed_tasks() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 10,
            description: "done".into(),
            tool_hint: None,
            completed: true,
        }];
        let mut sink = SilentEventSink;
        let r = handle_update_task_plan(
            r#"{"tasks":[{"description":"next step","completed":false}],"reason":"pivot"}"#,
            &mut planner,
            &[],
            &mut sink,
        );
        assert!(!r.is_error);
        assert_eq!(planner.task_list.len(), 2);
        assert!(planner.task_list[0].completed);
        assert_eq!(planner.task_list[0].id, 10);
        assert!(!planner.task_list[1].completed);
        assert_eq!(planner.task_list[1].id, 11);
        assert!(r.content.contains("Reason: pivot"));
    }

    #[test]
    fn handle_complete_task_errors_on_wrong_id() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id": 9, "completion_type":"success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(r.is_error);
        assert!(r.content.contains("current task"));
    }

    #[test]
    fn handle_complete_task_marks_current_done() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 3,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id":3,"summary":"ok","completion_type":"success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(!r.is_error);
        assert!(planner.task_list[0].completed);
        assert!(r.content.contains("\"task_id\": 3"));
        assert!(r.content.contains("\"completion_type\": \"success\""));
    }

    #[test]
    fn complete_task_accepts_partial_success_completion_type() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id":1,"completion_type":"partial_success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(!r.is_error, "{}", r.content);
        assert!(r
            .content
            .contains("\"completion_type\": \"partial_success\""));
    }

    #[test]
    fn complete_task_rejects_invalid_completion_type() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id":1,"completion_type":"unknown"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(r.is_error);
        assert!(r.content.contains("Invalid completion_type"));
    }

    // ── Long-task regression tests: LLM sends wrong JSON types ──────────────

    #[test]
    fn complete_task_accepts_task_id_as_string() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id": "1", "summary": "done", "completion_type":"success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(
            !r.is_error,
            "task_id as string \"1\" should be accepted: {}",
            r.content
        );
        assert!(planner.task_list[0].completed);
    }

    #[test]
    fn complete_task_rejects_non_numeric_string_task_id() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"task_id": "abc", "completion_type":"success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(r.is_error);
        assert!(
            r.content.contains("not a valid number"),
            "error should explain: {}",
            r.content
        );
    }

    #[test]
    fn complete_task_rejects_missing_task_id() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(
            r#"{"summary": "done", "completion_type":"success"}"#,
            &mut planner,
            &mut sink,
        );
        assert!(r.is_error);
        assert!(
            r.content.contains("Missing required field"),
            "{}",
            r.content
        );
    }

    #[test]
    fn complete_task_rejects_missing_completion_type() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let r = handle_complete_task(r#"{"task_id":1}"#, &mut planner, &mut sink);
        assert!(r.is_error);
        assert!(r
            .content
            .contains("Missing required field: completion_type"));
    }

    #[test]
    fn update_task_plan_accepts_tasks_as_stringified_json_array() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let args =
            r#"{"tasks": "[{\"id\": 1, \"description\": \"new task\"}]", "reason": "retry"}"#;
        let r = handle_update_task_plan(args, &mut planner, &[], &mut sink);
        assert!(
            !r.is_error,
            "stringified tasks array should be accepted: {}",
            r.content
        );
        assert!(!planner.task_list.is_empty());
        assert!(r.content.contains("Reason: retry"));
    }

    #[test]
    fn update_task_plan_rejects_non_array_string() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let args = r#"{"tasks": "not an array at all"}"#;
        let r = handle_update_task_plan(args, &mut planner, &[], &mut sink);
        assert!(r.is_error);
        assert!(r.content.contains("must be a JSON array"), "{}", r.content);
    }

    #[test]
    fn update_task_plan_rejects_missing_tasks_field() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let args = r#"{"reason": "just a reason"}"#;
        let r = handle_update_task_plan(args, &mut planner, &[], &mut sink);
        assert!(r.is_error);
        assert!(
            r.content.contains("Missing required field"),
            "{}",
            r.content
        );
    }

    // ── Double-encoded JSON unwrap tests ──────────────────────────────────

    #[test]
    fn complete_task_handles_double_encoded_json() {
        let mut planner = TaskPlanner::new(None, None, None);
        planner.task_list = vec![Task {
            id: 1,
            description: "a".into(),
            tool_hint: None,
            completed: false,
        }];
        let mut sink = SilentEventSink;
        let double_encoded =
            r#""{\"task_id\": 1, \"summary\": \"done\", \"completion_type\":\"success\"}""#;
        let r = handle_complete_task(double_encoded, &mut planner, &mut sink);
        assert!(
            !r.is_error,
            "double-encoded JSON should be unwrapped: {}",
            r.content
        );
        assert!(planner.task_list[0].completed);
    }

    #[test]
    fn update_task_plan_handles_double_encoded_json() {
        let mut planner = TaskPlanner::new(None, None, None);
        let mut sink = SilentEventSink;
        let double_encoded =
            r#""{\"tasks\": [{\"id\": 1, \"description\": \"new task\"}], \"reason\": \"retry\"}""#;
        let r = handle_update_task_plan(double_encoded, &mut planner, &[], &mut sink);
        assert!(
            !r.is_error,
            "double-encoded JSON should be unwrapped: {}",
            r.content
        );
        assert!(!planner.task_list.is_empty());
    }

    #[test]
    fn latest_trailing_tool_results_detects_substantive_weather_like_output() {
        let long_body: String = (0..120).map(|_| "x").collect();
        let msgs = vec![
            ChatMessage::user("深圳天气"),
            ChatMessage::assistant("ignored"),
            ChatMessage::tool_result("call_1", &long_body),
        ];
        assert!(latest_trailing_tool_results_include_substantive_output(&msgs, 80));
        let short = vec![
            ChatMessage::user("x"),
            ChatMessage::tool_result("c", "short"),
        ];
        assert!(!latest_trailing_tool_results_include_substantive_output(&short, 80));
        let multi = vec![
            ChatMessage::user("u"),
            ChatMessage::assistant("a"),
            ChatMessage::tool_result("c2", "{\"ok\":true}"),
            ChatMessage::tool_result("c1", &long_body),
        ];
        assert!(latest_trailing_tool_results_include_substantive_output(&multi, 80));
    }

    #[test]
    fn pick_substantive_trailing_tool_prefers_long_result_before_short_tail() {
        let long_body: String = (0..100).map(|_| "z").collect();
        let done = r#"{"success": true, "task_id": 1, "completion_type": "success", "message": "Task 1 marked as completed"}"#;
        let msgs = vec![
            ChatMessage::user("u"),
            ChatMessage::assistant("a"),
            ChatMessage::tool_result("weather", &long_body),
            ChatMessage::tool_result("done", done),
        ];
        let picked = pick_substantive_trailing_tool_content(&msgs, 80, TRAILING_TOOL_SUMMARY_MAX_BYTES)
            .expect("pick");
        assert_eq!(picked.len(), long_body.len());
    }

    #[test]
    fn pick_skips_complete_task_json_when_it_is_last_tool_message() {
        let weather = format!(
            "{}{}",
            "城市：深圳市\n温度：27°C\n湿度：70%\n",
            "x".repeat(80)
        );
        let done = r#"{"success": true, "task_id": 1, "completion_type": "success", "message": "Task 1 marked as completed"}"#;
        let msgs = vec![
            ChatMessage::user("u"),
            ChatMessage::assistant("a"),
            ChatMessage::tool_result("w", &weather),
            ChatMessage::tool_result("c", done),
        ];
        let picked = pick_substantive_trailing_tool_content(&msgs, 80, TRAILING_TOOL_SUMMARY_MAX_BYTES)
            .expect("pick");
        assert!(picked.contains("城市"));
        assert!(!picked.contains("completion_type"));
    }

    #[test]
    fn latest_trailing_does_not_treat_complete_task_json_as_substantive() {
        let done = r#"{"success": true, "task_id": 1, "completion_type": "success", "message": "Task 1 marked as completed"}"#;
        let msgs = vec![
            ChatMessage::user("u"),
            ChatMessage::tool_result("c", done),
        ];
        assert!(!latest_trailing_tool_results_include_substantive_output(&msgs, 80));
    }

    #[test]
    fn build_agent_result_falls_back_to_substantive_trailing_tool_output() {
        let weather_like: String = (0..120).map(|_| "y").collect();
        let messages = vec![
            ChatMessage::user("深圳天气"),
            ChatMessage::assistant_with_tool_calls(None, vec![]),
            ChatMessage::tool_result("w", &weather_like),
            ChatMessage::tool_result("c", r#"{"success":true}"#),
        ];
        let plan = vec![Task {
            id: 1,
            description: "查天气".into(),
            tool_hint: Some("weather".into()),
            completed: true,
        }];
        let out = build_agent_result(
            messages,
            1,
            3,
            plan,
            ExecutionFeedback {
                task_completed: true,
                ..ExecutionFeedback::default()
            },
        );
        assert_eq!(out.response.len(), weather_like.len());
        assert!(out.response.starts_with("yyy"));
    }

    #[test]
    fn build_agent_result_picks_last_assistant_text() {
        let messages = vec![
            ChatMessage::user("hi"),
            ChatMessage::assistant("first"),
            ChatMessage::assistant("final answer"),
        ];
        let plan = vec![Task {
            id: 1,
            description: "t".into(),
            tool_hint: None,
            completed: true,
        }];
        let out = build_agent_result(
            messages.clone(),
            2,
            4,
            plan.clone(),
            ExecutionFeedback {
                task_completed: true,
                ..ExecutionFeedback::default()
            },
        );
        assert_eq!(out.response, "final answer");
        assert_eq!(out.tool_calls_count, 2);
        assert_eq!(out.iterations, 4);
        assert_eq!(out.task_plan.len(), plan.len());
        assert_eq!(out.task_plan[0].id, plan[0].id);
    }

    #[test]
    fn build_agent_result_returns_unfinished_summary_when_plan_pending() {
        let messages = vec![
            ChatMessage::user("深圳明天的天气怎样"),
            ChatMessage::assistant("我只能查到当前天气"),
        ];
        let plan = vec![Task {
            id: 1,
            description: "查询深圳明天天气".into(),
            tool_hint: Some("weather".into()),
            completed: false,
        }];
        let out = build_agent_result(messages, 1, 2, plan, ExecutionFeedback::default());
        assert!(out.response.contains("任务未完成（0/1）"));
        assert!(out
            .response
            .contains("completion_type=\"partial_success|failure\""));
    }

    #[test]
    fn parse_goal_contract_json_maps_fields_and_risk() {
        let json = r#"{"goal":"Ship v1","acceptance":"all tests pass","constraints":"no new deps","deadline":"Friday","risk_level":"high"}"#;
        let c = parse_goal_contract_json(json).expect("should parse");
        assert_eq!(c.goal.as_deref(), Some("Ship v1"));
        assert_eq!(c.acceptance.as_deref(), Some("all tests pass"));
        assert_eq!(c.constraints.as_deref(), Some("no new deps"));
        assert_eq!(c.deadline.as_deref(), Some("Friday"));
        assert_eq!(c.risk_level, Some(RiskLevel::High));
    }

    #[test]
    fn parse_goal_contract_json_rejects_invalid_json() {
        let err = parse_goal_contract_json("{invalid json").expect_err("must fail");
        assert!(
            err.to_string().contains("Goal contract JSON parse error"),
            "{}",
            err
        );
    }
}
