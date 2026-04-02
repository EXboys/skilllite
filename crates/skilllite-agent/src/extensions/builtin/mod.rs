//! Built-in tools for the agent.
//!
//! Split into submodules by tool category:
//! - `file_ops`:    read_file, write_file, search_replace, insert_lines, grep_files, list_directory, file_exists
//! - `run_command`: run_command (shell execution with confirmation)
//! - `output`:      write_output, list_output
//! - `preview`:     preview_server (local HTTP file server)
//! - `chat_data`:   chat_history, chat_plan, update_task_plan
//!
//! This module provides shared security helpers, the tool definition registry,
//! and the dispatch layer that routes tool calls to the appropriate submodule.

mod chat_data;
mod delegate_swarm;
mod file_ops;
mod helpers;
mod output;
mod preview;
mod run_command;

#[cfg(test)]
mod tests;

use serde_json::Value;
use std::path::Path;

use crate::types::{self, safe_slice_from, safe_truncate, EventSink, ToolDefinition, ToolResult};
use helpers::*;

use super::registry::{RegisteredTool, ToolCapability, ToolHandler, ToolScope};

// ─── Tool definitions (aggregated from submodules) ───────────────────────────

pub fn get_builtin_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(file_ops::tool_definitions());
    tools.extend(run_command::tool_definitions());
    tools.extend(output::tool_definitions());
    tools.extend(preview::tool_definitions());
    tools.extend(chat_data::tool_definitions());
    tools.extend(delegate_swarm::tool_definitions());
    tools
}

/// Built-in tools paired with capability requirements and handlers.
pub fn get_builtin_tools() -> Vec<RegisteredTool> {
    get_builtin_tool_definitions()
        .into_iter()
        .map(|definition| {
            let name = definition.function.name.as_str();
            let capabilities = builtin_capabilities(name);
            let (handler, scope) = if matches!(name, "complete_task" | "update_task_plan") {
                (ToolHandler::PlanningControl, ToolScope::PlanningOnly)
            } else if is_async_builtin_tool(name) {
                (ToolHandler::BuiltinAsync, ToolScope::AllModes)
            } else {
                (ToolHandler::BuiltinSync, ToolScope::AllModes)
            };
            RegisteredTool::new(definition, capabilities, handler).with_scope(scope)
        })
        .collect()
}

fn builtin_capabilities(name: &str) -> Vec<ToolCapability> {
    match name {
        "write_file" | "search_replace" | "insert_lines" | "write_output" => {
            vec![ToolCapability::FilesystemWrite]
        }
        "run_command" => vec![ToolCapability::ProcessExec],
        "preview_server" => vec![ToolCapability::Preview],
        "delegate_to_swarm" => vec![ToolCapability::Delegation],
        _ => Vec::new(),
    }
}

// ─── Dispatch ────────────────────────────────────────────────────────────────

pub fn is_async_builtin_tool(name: &str) -> bool {
    matches!(name, "run_command" | "preview_server" | "delegate_to_swarm")
}

pub fn execute_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> ToolResult {
    let (args, was_recovered) = match serde_json::from_str(arguments) {
        Ok(v) => (v, false),
        Err(_e) => {
            if tool_name == "write_file" || tool_name == "write_output" {
                match parse_truncated_json_for_file_tools(arguments) {
                    Some(recovered) if recovered.as_object().is_some_and(|o| !o.is_empty()) => {
                        tracing::warn!(
                            "Recovered truncated JSON for {} ({} fields)",
                            tool_name,
                            recovered.as_object().map_or(0, |o| o.len())
                        );
                        (recovered, true)
                    }
                    _ => {
                        return ToolResult {
                            tool_call_id: String::new(),
                            tool_name: tool_name.to_string(),
                            content: format!("Invalid arguments JSON: {}", _e),
                            is_error: true,
                            counts_as_failure: true,
                        };
                    }
                }
            } else {
                return ToolResult {
                    tool_call_id: String::new(),
                    tool_name: tool_name.to_string(),
                    content: format!("Invalid arguments JSON: {}", _e),
                    is_error: true,
                    counts_as_failure: true,
                };
            }
        }
    };

    let result = match tool_name {
        "read_file" => file_ops::execute_read_file(&args, workspace),
        "write_file" => file_ops::execute_write_file(&args, workspace, event_sink),
        "search_replace" => file_ops::execute_search_replace(&args, workspace, event_sink),
        "preview_edit" => file_ops::execute_preview_edit(&args, workspace),
        "insert_lines" => file_ops::execute_insert_lines(&args, workspace, event_sink),
        "grep_files" => file_ops::execute_grep_files(&args, workspace),
        "list_directory" => file_ops::execute_list_directory(&args, workspace),
        "file_exists" => file_ops::execute_file_exists(&args, workspace),
        "write_output" => output::execute_write_output(&args, workspace),
        "chat_history" => chat_data::execute_chat_history(&args),
        "chat_plan" => chat_data::execute_chat_plan(&args),
        "list_output" => output::execute_list_output(&args),
        "update_task_plan" | "complete_task" => Err(crate::Error::validation(format!(
            "{} is a planning control tool; it must be dispatched via registry.execute with planning_ctx",
            tool_name
        ))),
        "delegate_to_swarm" => Err(crate::Error::validation(
            "delegate_to_swarm is async; it must be handled by the agent loop".to_string()
        )),
        _ => Err(crate::Error::validation(format!("Unknown built-in tool: {}", tool_name))),
    };

    match result {
        Ok(content) => {
            let final_content =
                if was_recovered && (tool_name == "write_file" || tool_name == "write_output") {
                    format!(
                        "{}\n\n⚠️ Content may have been truncated due to token limit. \
                     Consider splitting into smaller chunks or verify the output. \
                     Increase SKILLLITE_MAX_TOKENS if needed.",
                        content
                    )
                } else {
                    content
                };
            ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: final_content,
                is_error: false,
                counts_as_failure: false,
            }
        }
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
            counts_as_failure: true,
        },
    }
}

pub async fn execute_async_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Invalid arguments JSON: {}", e),
                is_error: true,
                counts_as_failure: true,
            };
        }
    };

    if tool_name == "run_command" {
        return match run_command::execute_run_command(&args, workspace, event_sink).await {
            Ok(outcome) => ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: outcome.content,
                is_error: outcome.is_error,
                counts_as_failure: outcome.counts_as_failure,
            },
            Err(e) => ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Error: {}", e),
                is_error: true,
                counts_as_failure: true,
            },
        };
    }

    let result = match tool_name {
        "preview_server" => preview::execute_preview_server(&args, workspace, event_sink),
        "delegate_to_swarm" => {
            delegate_swarm::execute_delegate_to_swarm(&args, workspace, event_sink).await
        }
        _ => Err(crate::Error::validation(format!(
            "Unknown async built-in tool: {}",
            tool_name
        ))),
    };

    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
            counts_as_failure: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
            counts_as_failure: true,
        },
    }
}

// ─── Long content handling ──────────────────────────────────────────────────

pub fn process_tool_result_content(content: &str) -> Option<String> {
    let max_chars = types::get_tool_result_max_chars();
    let summarize_threshold = types::get_summarize_threshold();
    let len = content.len();

    if len <= max_chars {
        return Some(content.to_string());
    }

    if len > summarize_threshold {
        return None;
    }

    Some(format!(
        "{}\n\n[... 结果已截断，原文共 {} 字符，仅保留前 {} 字符 ...]",
        safe_truncate(content, max_chars),
        len,
        max_chars
    ))
}

/// Head+tail truncation with an explicit byte budget (used for `read_file` vs other tools).
pub fn process_tool_result_content_fallback_with_max(content: &str, max_chars: usize) -> String {
    let len = content.len();
    if len <= max_chars {
        return content.to_string();
    }

    let head_size = max_chars.min(len);
    let tail_size = (max_chars / 3).min(len);
    let head = safe_truncate(content, head_size);
    let tail = safe_slice_from(content, len.saturating_sub(tail_size));
    format!(
        "{}\n\n... [content truncated: {} chars total, showing head+tail] ...\n\n{}",
        head, len, tail
    )
}

pub fn process_tool_result_content_fallback(content: &str) -> String {
    process_tool_result_content_fallback_with_max(content, types::get_tool_result_max_chars())
}

/// `read_file` only: large dedicated cap; never LLM-summarized (see `process_result_content`).
pub fn process_read_file_tool_result_content(content: &str) -> String {
    let cap = types::get_read_file_tool_result_max_chars();
    let len = content.len();
    if len <= cap {
        return content.to_string();
    }
    tracing::info!(
        "read_file tool result {} chars exceeds SKILLLITE_READ_FILE_TOOL_RESULT_MAX_CHARS ({}), using head+tail truncation",
        len,
        cap
    );
    process_tool_result_content_fallback_with_max(content, cap)
}
