//! Shared helpers for the agent loop: tool execution, result processing,
//! progressive disclosure, task plan handling, and result building.

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use anyhow::Result;

use super::super::extensions::{self};
use super::super::goal_boundaries::{self, GoalBoundaries};
use super::super::llm::LlmClient;
use super::super::long_text;
use super::super::prompt;
use super::super::skills::{self, LoadedSkill};
use super::super::task_planner::TaskPlanner;
use super::super::types::*;

/// Handle update_task_plan: parse new tasks, replace planner.task_list, notify event_sink.
pub(super) fn handle_update_task_plan(
    arguments: &str,
    planner: &mut TaskPlanner,
    event_sink: &mut dyn EventSink,
) -> super::super::types::ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return super::super::types::ToolResult {
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
            return super::super::types::ToolResult {
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
        return super::super::types::ToolResult {
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
    super::super::types::ToolResult {
        tool_call_id: String::new(),
        tool_name: "update_task_plan".to_string(),
        content,
        is_error: false,
    }
}

/// Execute a single tool call via ExtensionRegistry.
pub(super) async fn execute_tool_call(
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

/// Extract goal boundaries via LLM (fallback when regex returns empty).
/// Enabled by SKILLLITE_GOAL_LLM_EXTRACT=1.
pub(super) async fn extract_goal_boundaries_llm(
    client: &LlmClient,
    model: &str,
    goal: &str,
) -> Result<GoalBoundaries> {
    const PROMPT: &str = r#"Extract goal boundaries from the user's goal. Return JSON only:
{"scope": "...", "exclusions": "...", "completion_conditions": "..."}
- scope: what is in scope for this goal (optional, null if unclear)
- exclusions: what to avoid or exclude (optional, null if unclear)
- completion_conditions: when the task is considered done (optional, null if unclear)
Use null for any field you cannot infer. Output only valid JSON, no markdown, no other text."#;

    let messages = vec![
        ChatMessage::system(PROMPT),
        ChatMessage::user(goal),
    ];

    let resp = client
        .chat_completion(model, &messages, None, Some(0.2))
        .await?;

    let raw = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    let raw = raw.trim();
    let json_str = if raw.starts_with("```json") {
        raw.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim()
    } else if raw.starts_with("```") {
        raw.trim_start_matches("```").trim_end_matches("```").trim()
    } else {
        raw
    };

    let v: Value = serde_json::from_str(json_str).map_err(|e| anyhow::anyhow!("Goal boundaries JSON parse error: {}", e))?;

    let scope = v.get("scope").and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let exclusions = v.get("exclusions").and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let completion_conditions = v.get("completion_conditions").and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    Ok(GoalBoundaries {
        scope,
        exclusions,
        completion_conditions,
    })
}

/// Hybrid extraction: regex first, LLM fallback when regex returns empty.
/// LLM fallback only when SKILLLITE_GOAL_LLM_EXTRACT=1.
pub(super) async fn extract_goal_boundaries_hybrid(
    client: &LlmClient,
    model: &str,
    goal: &str,
) -> Result<GoalBoundaries> {
    let regex_result = goal_boundaries::extract_goal_boundaries(goal);
    if !regex_result.is_empty() {
        return Ok(regex_result);
    }
    if std::env::var("SKILLLITE_GOAL_LLM_EXTRACT").as_deref() == Ok("1") {
        tracing::info!("Goal boundaries regex empty, trying LLM extraction");
        extract_goal_boundaries_llm(client, model, goal).await
    } else {
        Ok(regex_result)
    }
}

/// Build the final `AgentResult` from the message history.
pub(super) fn build_agent_result(
    messages: Vec<ChatMessage>,
    tool_calls_count: usize,
    iterations: usize,
    task_plan: Vec<Task>,
    feedback: ExecutionFeedback,
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
        feedback,
    }
}

