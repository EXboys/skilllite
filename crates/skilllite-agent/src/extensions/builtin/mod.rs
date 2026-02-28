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

mod file_ops;
mod helpers;
mod run_command;
mod output;
mod preview;
mod chat_data;

#[cfg(test)]
mod tests;

use serde_json::Value;
use std::path::Path;

use crate::types::{self, EventSink, ToolDefinition, ToolResult, safe_truncate, safe_slice_from};
use helpers::*;

// ─── Tool definitions (aggregated from submodules) ───────────────────────────

pub fn get_builtin_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(file_ops::tool_definitions());
    tools.extend(run_command::tool_definitions());
    tools.extend(output::tool_definitions());
    tools.extend(preview::tool_definitions());
    tools.extend(chat_data::tool_definitions());
    tools
}

// ─── Dispatch ────────────────────────────────────────────────────────────────

pub fn is_builtin_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "write_file"
            | "search_replace"
            | "preview_edit"
            | "insert_lines"
            | "grep_files"
            | "list_directory"
            | "file_exists"
            | "run_command"
            | "write_output"
            | "preview_server"
            | "chat_history"
            | "chat_plan"
            | "list_output"
            | "update_task_plan"
    )
}

pub fn is_async_builtin_tool(name: &str) -> bool {
    matches!(name, "run_command" | "preview_server")
}

pub fn execute_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
) -> ToolResult {
    let (args, was_recovered) = match serde_json::from_str(arguments) {
        Ok(v) => (v, false),
        Err(_e) => {
            if tool_name == "write_file" || tool_name == "write_output" {
                match parse_truncated_json_for_file_tools(arguments) {
                    Some(recovered) if recovered.as_object().map_or(false, |o| !o.is_empty()) => {
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
                        };
                    }
                }
            } else {
                return ToolResult {
                    tool_call_id: String::new(),
                    tool_name: tool_name.to_string(),
                    content: format!("Invalid arguments JSON: {}", _e),
                    is_error: true,
                };
            }
        }
    };

    let result = match tool_name {
        "read_file" => file_ops::execute_read_file(&args, workspace),
        "write_file" => file_ops::execute_write_file(&args, workspace),
        "search_replace" => file_ops::execute_search_replace(&args, workspace),
        "preview_edit" => file_ops::execute_preview_edit(&args, workspace),
        "insert_lines" => file_ops::execute_insert_lines(&args, workspace),
        "grep_files" => file_ops::execute_grep_files(&args, workspace),
        "list_directory" => file_ops::execute_list_directory(&args, workspace),
        "file_exists" => file_ops::execute_file_exists(&args, workspace),
        "write_output" => output::execute_write_output(&args, workspace),
        "chat_history" => chat_data::execute_chat_history(&args),
        "chat_plan" => chat_data::execute_chat_plan(&args),
        "list_output" => output::execute_list_output(&args),
        "update_task_plan" => Err(anyhow::anyhow!(
            "update_task_plan is only available in task-planning mode; it must be handled by the agent loop"
        )),
        _ => Err(anyhow::anyhow!("Unknown built-in tool: {}", tool_name)),
    };

    match result {
        Ok(content) => {
            let final_content = if was_recovered
                && (tool_name == "write_file" || tool_name == "write_output")
            {
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
            }
        }
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
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
            };
        }
    };

    let result = match tool_name {
        "run_command" => run_command::execute_run_command(&args, workspace, event_sink).await,
        "preview_server" => preview::execute_preview_server(&args, workspace),
        _ => Err(anyhow::anyhow!("Unknown async built-in tool: {}", tool_name)),
    };

    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
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

pub fn process_tool_result_content_fallback(content: &str) -> String {
    let max_chars = types::get_tool_result_max_chars();
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

