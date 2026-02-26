//! Built-in tools for the agent.
//!
//! Split into submodules by tool category:
//! - `file_ops`:    read_file, write_file, search_replace, list_directory, file_exists
//! - `run_command`: run_command (shell execution with confirmation)
//! - `output`:      write_output, list_output
//! - `preview`:     preview_server (local HTTP file server)
//! - `chat_data`:   chat_history, chat_plan, update_task_plan
//!
//! This module provides shared security helpers, the tool definition registry,
//! and the dispatch layer that routes tool calls to the appropriate submodule.

mod file_ops;
mod run_command;
mod output;
mod preview;
mod chat_data;

use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::types::{self, EventSink, ToolDefinition, ToolResult, safe_truncate, safe_slice_from};

// ─── Security helpers (shared by submodules via super::) ─────────────────────

const SENSITIVE_PATTERNS: &[&str] = &[".env", ".git/config", ".key"];

fn is_sensitive_write_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.ends_with(pattern) || lower.contains(&format!("{}/", pattern)) {
            return true;
        }
    }
    if lower.ends_with(".key") || lower.ends_with(".pem") {
        return true;
    }
    false
}

fn resolve_within_workspace(path: &str, workspace: &Path) -> Result<PathBuf> {
    let input = Path::new(path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        workspace.join(input)
    };

    let normalized = normalize_path(&resolved);

    if !normalized.starts_with(workspace) {
        let is_output_path = types::get_output_dir()
            .map_or(false, |od| normalized.starts_with(Path::new(&od)));
        if is_output_path {
            anyhow::bail!(
                "Path escapes workspace: {} (workspace: {}). \
                 Hint: this path is in the output directory — use **write_output** \
                 (with file_path relative to the output dir) instead of write_file.",
                path,
                workspace.display()
            );
        } else {
            anyhow::bail!(
                "Path escapes workspace: {} (workspace: {})",
                path,
                workspace.display()
            );
        }
    }

    Ok(normalized)
}

fn resolve_within_workspace_or_output(path: &str, workspace: &Path) -> Result<PathBuf> {
    if let Ok(resolved) = resolve_within_workspace(path, workspace) {
        return Ok(resolved);
    }

    if let Some(output_dir) = types::get_output_dir() {
        let output_root = PathBuf::from(&output_dir);
        let input = Path::new(path);
        let resolved = if input.is_absolute() {
            input.to_path_buf()
        } else {
            output_root.join(input)
        };
        let normalized = normalize_path(&resolved);
        if normalized.starts_with(&output_root) {
            return Ok(normalized);
        }
    }

    anyhow::bail!(
        "Path escapes workspace: {} (workspace: {})",
        path,
        workspace.display()
    )
}

fn get_path_arg(args: &Value, for_directory: bool) -> Option<String> {
    let path = args.get("path").and_then(|v| v.as_str());
    let alt = if for_directory {
        args.get("directory_path").and_then(|v| v.as_str())
    } else {
        args.get("file_path").and_then(|v| v.as_str())
    };
    path.or(alt).map(String::from)
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

// ─── Shared directory listing (used by file_ops and output) ─────────────────

fn list_dir_impl(
    base: &Path,
    current: &Path,
    recursive: bool,
    entries: &mut Vec<String>,
    depth: usize,
) -> Result<()> {
    let mut items: Vec<_> = std::fs::read_dir(current)?
        .filter_map(|e| e.ok())
        .collect();
    items.sort_by_key(|e| e.file_name());

    let skip_dirs = [
        "node_modules",
        "__pycache__",
        ".git",
        "venv",
        ".venv",
        ".tox",
        "target",
    ];

    for entry in items {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && depth == 0 && name != "." {
            let prefix = if entry.path().is_dir() { "📁 " } else { "   " };
            entries.push(format!("{}{}", prefix, name));
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(base)
            .unwrap_or(&entry.path())
            .to_string_lossy()
            .to_string();

        if entry.path().is_dir() {
            entries.push(format!("📁 {}/", rel));
            if recursive && !skip_dirs.contains(&name.as_str()) {
                list_dir_impl(base, &entry.path(), true, entries, depth + 1)?;
            }
        } else {
            let meta = entry.metadata().ok();
            let size = meta.map(|m| m.len()).unwrap_or(0);
            entries.push(format!("   {} ({})", rel, format_size(size)));
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ─── Truncated JSON recovery ─────────────────────────────────────────────────

fn parse_truncated_json_for_file_tools(arguments: &str) -> Option<Value> {
    if arguments.is_empty() {
        return None;
    }

    let mut result = serde_json::Map::new();

    if arguments.contains("\"append\":true") {
        result.insert("append".to_string(), Value::Bool(true));
    } else if arguments.contains("\"append\":false") {
        result.insert("append".to_string(), Value::Bool(false));
    }

    let path_re = regex::Regex::new(r#""(?:file_)?path"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = path_re.captures(arguments) {
        let key = if arguments.contains("\"file_path\"") {
            "file_path"
        } else {
            "path"
        };
        result.insert(
            key.to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    }

    let content_complete_re =
        regex::Regex::new(r#""content"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = content_complete_re.captures(arguments) {
        result.insert(
            "content".to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    } else {
        let content_trunc_re = regex::Regex::new(r#""content"\s*:\s*"(.*)$"#).ok()?;
        if let Some(caps) = content_trunc_re.captures(arguments) {
            let mut raw = caps.get(1)?.as_str().to_string();
            if raw.ends_with("\"}") {
                raw = raw[..raw.len() - 2].to_string();
            } else if raw.ends_with('"') && !raw.ends_with("\\\"") {
                raw = raw[..raw.len() - 1].to_string();
            }
            result.insert(
                "content".to_string(),
                Value::String(unescape_json_string(&raw)),
            );
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(Value::Object(result))
    }
}

fn unescape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_replace_first_occurrence() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "hello world\nhello again\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "hello world",
            "new_string": "hi world",
            "replace_all": false
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);
        assert!(result.content.contains("Successfully replaced 1 occurrence"));
        assert!(result.content.contains("\"first_changed_line\": 1"));
        assert!(result.content.contains("\"changed\": true"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi world\nhello again\n");
    }

    #[test]
    fn test_search_replace_requires_unique_match_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "hello world\nhello again\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "hello",
            "new_string": "hi"
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(result.is_error);
        assert!(result.content.contains("requires a unique match by default"));
    }

    #[test]
    fn test_search_replace_all() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "foo bar foo baz foo\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "foo",
            "new_string": "qux",
            "replace_all": true
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);
        assert!(result.content.contains("Successfully replaced 3 occurrence"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "qux bar qux baz qux\n");
    }

    #[test]
    fn test_search_replace_old_string_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "hello world\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "xyz",
            "new_string": "abc"
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(result.is_error);
        assert!(result.content.contains("old_string not found"));
    }

    #[test]
    fn test_search_replace_blocks_sensitive_path() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let env_path = workspace.join(".env");
        std::fs::write(&env_path, "KEY=value\n").unwrap();

        let args = serde_json::json!({
            "path": ".env",
            "old_string": "KEY=value",
            "new_string": "KEY=modified"
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(result.is_error);
        assert!(result.content.contains("Blocked"));
    }

    #[test]
    fn test_search_replace_normalize_whitespace_trailing() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "hello world  \nnext line\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "hello world",
            "new_string": "hi",
            "normalize_whitespace": true
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi\nnext line\n");
    }

    #[test]
    fn test_search_replace_normalize_whitespace_replace_all() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "foo \nbar \nbaz\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "bar",
            "new_string": "qux",
            "replace_all": true,
            "normalize_whitespace": true
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "foo \nqux\nbaz\n");
    }

    #[test]
    fn test_search_replace_normalize_whitespace_literal_replacement() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "price: 100\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "price: 100",
            "new_string": "price: $200",
            "normalize_whitespace": true
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "price: $200\n");
    }

    #[test]
    fn test_search_replace_output_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let output_dir = workspace.join("output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let file_path = output_dir.join("index.html");
        std::fs::write(&file_path, "<title>Old Title</title>").unwrap();

        let args = serde_json::json!({
            "path": "output/index.html",
            "old_string": "Old Title",
            "new_string": "New Title"
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "<title>New Title</title>");
    }

    #[test]
    fn test_preview_edit_does_not_write_file() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path();
        let file_path = workspace.join("test.txt");
        std::fs::write(&file_path, "alpha beta\n").unwrap();

        let args = serde_json::json!({
            "path": "test.txt",
            "old_string": "alpha",
            "new_string": "gamma"
        });
        let result = execute_builtin_tool("preview_edit", &args.to_string(), workspace);
        assert!(!result.is_error);
        assert!(result.content.contains("Preview edit"));
        assert!(result.content.contains("\"changed\": true"));
        assert!(result.content.contains("\"diff_excerpt\""));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "alpha beta\n");
    }
}
