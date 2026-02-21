//! Built-in tools for the agent (file ops, run_command, output, preview, chat).
//!
//! Phase 1: read_file, write_file, search_replace, list_directory, file_exists
//! Phase 2: run_command, write_output, preview_server
//! Phase 3: chat_history, chat_plan, list_output, update_task_plan
//!
//! Ported from Python `builtin_tools.py`. Enforces workspace confinement
//! and sensitive path blocking.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::agent::types::{self, EventSink, ToolDefinition, FunctionDef, ToolResult, safe_truncate, safe_slice_from};

// ─── Security helpers (ported from Python) ──────────────────────────────────

/// Sensitive file patterns that should never be written to.
const SENSITIVE_PATTERNS: &[&str] = &[".env", ".git/config", ".key"];

/// Check if a path is sensitive and should be blocked for writes.
fn is_sensitive_write_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.ends_with(pattern) || lower.contains(&format!("{}/", pattern)) {
            return true;
        }
    }
    // Also block *.key files
    if lower.ends_with(".key") || lower.ends_with(".pem") {
        return true;
    }
    false
}

/// Resolve a path and ensure it stays within the workspace root.
/// Prevents path traversal attacks (e.g. "../../etc/passwd").
fn resolve_within_workspace(path: &str, workspace: &Path) -> Result<PathBuf> {
    let input = Path::new(path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        workspace.join(input)
    };

    // Normalize by resolving ".." components without requiring the path to exist
    let normalized = normalize_path(&resolved);

    if !normalized.starts_with(workspace) {
        // Check if the target looks like it's in the output directory
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

/// Extract path from tool args. Accepts both `path` and `file_path`/`directory_path` for Python SDK compatibility.
fn get_path_arg(args: &Value, for_directory: bool) -> Option<String> {
    let path = args.get("path").and_then(|v| v.as_str());
    let alt = if for_directory {
        args.get("directory_path").and_then(|v| v.as_str())
    } else {
        args.get("file_path").and_then(|v| v.as_str())
    };
    path.or(alt).map(String::from)
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
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

// ─── Tool definitions ───────────────────────────────────────────────────────

/// Get all built-in tool definitions in OpenAI function-calling format.
pub fn get_builtin_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "read_file".to_string(),
                description: "Read the contents of a file. Returns UTF-8 text content.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "write_file".to_string(),
                description: "Write content to a file. Creates parent directories if needed. Blocks writes to sensitive files (.env, .key, .git/config). Use append: true to append to existing file instead of overwriting.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append content to end of file. Default: false (overwrite)."
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "search_replace".to_string(),
                description: "Replace exact text in a file. Use for precise edits instead of read_file + write_file. Supports workspace and output directory (path relative to workspace or output dir). old_string must match exactly (including whitespace). If old_string appears multiple times, use replace_all: true to replace all, or false (default) to replace only the first occurrence.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact text to find and replace (must match including whitespace)"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Text to replace old_string with"
                        },
                        "replace_all": {
                            "type": "boolean",
                            "description": "If true, replace all occurrences. Default: false (replace first only)."
                        }
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "list_directory".to_string(),
                description: "List files and directories in a given path. Supports recursive listing.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path (relative to workspace or absolute). Defaults to workspace root."
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, list recursively. Default: false."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "file_exists".to_string(),
                description: "Check if a file or directory exists. Returns type (file/directory) and size.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to check"
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        // ── Phase 2 tools ──────────────────────────────────────────────
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_command".to_string(),
                description: "Execute a shell command in the workspace directory. Requires user confirmation before execution. Dangerous commands (rm -rf, curl|bash, etc.) are flagged with extra warnings. Timeout: 300 seconds.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "write_output".to_string(),
                description: "Write final output to the output directory. Use for deliverable files (HTML, reports, etc.). Path is relative to the output directory. Use append: true to append to existing file. For content >~6k chars, split into multiple calls: first call overwrites, subsequent calls use append: true.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "File path relative to the output directory"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write"
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append content to end of file. Default: false (overwrite)."
                        }
                    },
                    "required": ["file_path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "preview_server".to_string(),
                description: "Start a local HTTP server to preview HTML files in the browser. Specify the directory to serve.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Directory to serve (relative to workspace). Also accepts 'path'."
                        },
                        "path": {
                            "type": "string",
                            "description": "Alias for directory_path"
                        },
                        "port": {
                            "type": "integer",
                            "description": "Port number (default: 8765)"
                        },
                        "open_browser": {
                            "type": "boolean",
                            "description": "Whether to open browser automatically (default: true)",
                            "default": true
                        }
                    },
                    "required": []
                }),
            },
        },
        // ── Chat data read tools (no list_directory permission expansion) ──
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "chat_history".to_string(),
                description: "Read chat history from the session. Use when the user asks to view, summarize, or analyze past conversations. Returns messages in chronological order. The transcript may contain [compaction] entries — these are summaries from /compact (history compression). To analyze /compact effect, read the transcript and find the [compaction] block.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "date": {
                            "type": "string",
                            "description": "Optional. Date to read (YYYY-MM-DD or YYYYMMDD). If omitted, returns all available history. For 昨天/yesterday, use (today - 1 day). Check system prompt for current date."
                        },
                        "session_key": {
                            "type": "string",
                            "description": "Optional. Use the session_key from system prompt (default: 'default'). For current interactive chat, use that value; do NOT use 'memory' — that is a different concept."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "chat_plan".to_string(),
                description: "Read the task plan for a session. Use when the user asks about today's plan, task status, or what was planned.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "date": {
                            "type": "string",
                            "description": "Optional. Date (YYYY-MM-DD or YYYYMMDD). Default: today."
                        },
                        "session_key": {
                            "type": "string",
                            "description": "Optional. Session key (default: 'default')."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "list_output".to_string(),
                description: "List files in the output directory (where write_output saves files). Use when the user asks what files were generated, or to find output files by name. No path needed.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, list recursively. Default: false."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "update_task_plan".to_string(),
                description: "Revise the task plan when current tasks are unusable (e.g. chat_history returned irrelevant data for a city comparison). Call with the new task list. Use when: (1) a task's result is clearly not useful for the user's goal; (2) the plan was wrong (e.g. used chat_history for place comparison). Pass `tasks` array with id, description, tool_hint, completed.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "tasks": {
                            "type": "array",
                            "description": "New task list. Each task: {id, description, tool_hint?, completed: false}",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {"type": "number"},
                                    "description": {"type": "string"},
                                    "tool_hint": {"type": "string"},
                                    "completed": {"type": "boolean", "default": false}
                                },
                                "required": ["id", "description"]
                            }
                        },
                        "reason": {
                            "type": "string",
                            "description": "Brief reason for the plan revision (e.g. chat_history had no relevant city data)"
                        }
                    },
                    "required": ["tasks"]
                }),
            },
        },
    ]
}

// ─── Tool execution ─────────────────────────────────────────────────────────

/// Check if a tool name is a built-in tool.
pub fn is_builtin_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "write_file"
            | "search_replace"
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

/// Check if a built-in tool requires async execution (uses EventSink).
pub fn is_async_builtin_tool(name: &str) -> bool {
    matches!(name, "run_command" | "preview_server")
}

/// Execute a synchronous built-in tool. Returns the result content string.
/// For async tools (run_command, preview_server), use `execute_async_builtin_tool`.
pub fn execute_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
) -> ToolResult {
    let (args, was_recovered) = match serde_json::from_str(arguments) {
        Ok(v) => (v, false),
        Err(_e) => {
            // Truncated JSON recovery: when LLM hits max_tokens (finish_reason: "length"),
            // tool arguments may be cut off mid-string. Try to recover file_path + content
            // for write_file and write_output. Ported from Python `_parse_truncated_json_for_file_tools`.
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
        "read_file" => execute_read_file(&args, workspace),
        "write_file" => execute_write_file(&args, workspace),
        "search_replace" => execute_search_replace(&args, workspace),
        "list_directory" => execute_list_directory(&args, workspace),
        "file_exists" => execute_file_exists(&args, workspace),
        "write_output" => execute_write_output(&args, workspace),
        "chat_history" => execute_chat_history(&args),
        "chat_plan" => execute_chat_plan(&args),
        "list_output" => execute_list_output(&args),
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

/// Execute an async built-in tool (run_command, preview_server).
/// These tools require `EventSink` for user confirmation or streaming output.
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
        "run_command" => execute_run_command(&args, workspace, event_sink).await,
        "preview_server" => execute_preview_server(&args, workspace),
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

/// Read a file's UTF-8 content.
fn execute_read_file(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;

    // Allow reading from both workspace and output directory
    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }

    if resolved.is_dir() {
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    // Try reading as UTF-8, fall back to noting it's binary
    match std::fs::read_to_string(&resolved) {
        Ok(content) => Ok(content),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidData {
                let meta = std::fs::metadata(&resolved)?;
                Ok(format!(
                    "[Binary file, {} bytes. Cannot display as text.]",
                    meta.len()
                ))
            } else {
                Err(e.into())
            }
        }
    }
}

/// Write content to a file. Blocks sensitive paths.
fn execute_write_file(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;
    let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_sensitive_write_path(&path_str) {
        anyhow::bail!(
            "Blocked: writing to sensitive file '{}' is not allowed",
            path_str
        );
    }

    let resolved = resolve_within_workspace(&path_str, workspace)?;

    // Create parent directories
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    if append {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&resolved)
            .with_context(|| format!("Failed to open file for append: {}", resolved.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("Failed to append to file: {}", resolved.display()))?;
    } else {
        std::fs::write(&resolved, content)
            .with_context(|| format!("Failed to write file: {}", resolved.display()))?;
    }

    Ok(format!(
        "Successfully {} {} bytes to {}",
        if append { "appended" } else { "wrote" },
        content.len(),
        path_str
    ))
}

/// Replace exact text in a file. Cursor-style precise edit.
fn execute_search_replace(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;
    let old_string = args
        .get("old_string")
        .and_then(|v| v.as_str())
        .context("'old_string' is required")?;
    let new_string = args
        .get("new_string")
        .and_then(|v| v.as_str())
        .context("'new_string' is required")?;
    let replace_all = args.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_sensitive_write_path(&path_str) {
        anyhow::bail!(
            "Blocked: editing sensitive file '{}' is not allowed",
            path_str
        );
    }

    // Allow both workspace and output directory (same as read_file)
    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    let content = std::fs::read_to_string(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;

    let count = if old_string.is_empty() {
        anyhow::bail!("old_string cannot be empty");
    } else if replace_all {
        content.matches(old_string).count()
    } else {
        if content.contains(old_string) {
            1
        } else {
            0
        }
    };

    if count == 0 {
        anyhow::bail!(
            "old_string not found in file. Ensure it matches exactly (including whitespace and newlines)."
        );
    }

    let new_content = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };

    std::fs::write(&resolved, &new_content)
        .with_context(|| format!("Failed to write file: {}", path_str))?;

    Ok(format!(
        "Successfully replaced {} occurrence(s) in {}",
        count,
        path_str
    ))
}

/// List directory contents.
fn execute_list_directory(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, true).unwrap_or_else(|| ".".to_string());
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Allow listing both workspace and output directory
    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("Directory not found: {}", path_str);
    }
    if !resolved.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path_str);
    }

    let mut entries = Vec::new();
    list_dir_impl(&resolved, &resolved, recursive, &mut entries, 0)?;

    Ok(entries.join("\n"))
}

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

    // Skip hidden and common non-essential directories
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
            // Show top-level hidden dirs but don't recurse
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

/// Check if a file or directory exists.
fn execute_file_exists(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;

    // Allow checking both workspace and output directory
    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        return Ok(format!("{}: does not exist", path_str));
    }

    let meta = std::fs::metadata(&resolved)?;
    if meta.is_dir() {
        Ok(format!("{}: directory", path_str))
    } else {
        Ok(format!("{}: file ({} bytes)", path_str, meta.len()))
    }
}

/// Format byte size into human-readable string.
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

/// Attempt to extract file_path/path and content from truncated JSON.
/// When LLM hits max_tokens (finish_reason: "length"), tool arguments may be cut off.
/// Ported from Python `_parse_truncated_json_for_file_tools` in `core/tools.py`.
fn parse_truncated_json_for_file_tools(arguments: &str) -> Option<Value> {
    if arguments.is_empty() {
        return None;
    }

    let mut result = serde_json::Map::new();

    // Extract "append": true/false (optional)
    if arguments.contains("\"append\":true") {
        result.insert("append".to_string(), Value::Bool(true));
    } else if arguments.contains("\"append\":false") {
        result.insert("append".to_string(), Value::Bool(false));
    }

    // Extract "path" or "file_path": "value"
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

    // Extract "content": try complete JSON string first
    let content_complete_re =
        regex::Regex::new(r#""content"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = content_complete_re.captures(arguments) {
        result.insert(
            "content".to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    } else {
        // Truncated: match from "content": " to end of string
        let content_trunc_re = regex::Regex::new(r#""content"\s*:\s*"(.*)$"#).ok()?;
        if let Some(caps) = content_trunc_re.captures(arguments) {
            let mut raw = caps.get(1)?.as_str().to_string();
            // Strip trailing "}" or " that may be from truncated JSON structure
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

/// Unescape a JSON string value (handles \n, \", \\, \t, \r).
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

// ─── Phase 2: run_command ────────────────────────────────────────────────────

/// Dangerous command regex patterns.
/// Ported from Python `_check_dangerous_command`.
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*--force)", "rm with force flag — may delete files irreversibly"),
    (r"rm\s+-[a-zA-Z]*r[a-zA-Z]*\s+/\s*$", "rm -rf / — system destruction"),
    (r"(curl|wget)\s+.*\|\s*(bash|sh|zsh)", "piping remote script to shell — remote code execution risk"),
    (r":\(\)\s*\{\s*:\|:\s*&\s*\}\s*;\s*:", "fork bomb — will crash the system"),
    (r"chmod\s+(-[a-zA-Z]*R|--recursive)\s+777", "recursive chmod 777 — insecure permission change"),
];

/// Check if a command is dangerous. Returns a warning reason if so.
fn check_dangerous_command(cmd: &str) -> Option<String> {
    for (pattern, reason) in DANGEROUS_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(cmd) {
                return Some(reason.to_string());
            }
        }
    }
    None
}

/// Execute `run_command`: shell command with confirmation + timeout.
/// Ported from Python `builtin_tools.py` run_command implementation.
async fn execute_run_command(
    args: &Value,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> Result<String> {
    let cmd = args
        .get("command")
        .and_then(|v| v.as_str())
        .context("'command' is required")?;

    if cmd.trim().is_empty() {
        anyhow::bail!("command must not be empty");
    }

    // Build confirmation message
    let confirm_msg = if let Some(danger_reason) = check_dangerous_command(cmd) {
        format!(
            "⚠️ Dangerous command detected\n\n\
             Pattern that may cause serious harm: {}\n\n\
             Command: {}\n\n\
             Please verify before confirming execution.",
            danger_reason, cmd
        )
    } else {
        format!("About to execute command:\n  {}\n\nConfirm execution?", cmd)
    };

    // Request user confirmation
    if !event_sink.on_confirmation_request(&confirm_msg) {
        return Ok("User cancelled command execution".to_string());
    }

    // Execute command via tokio subprocess
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(workspace)
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;

    // Read stdout + stderr concurrently, stream to event_sink
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut output_lines = Vec::new();

    // Read stdout
    if let Some(stdout) = stdout {
        let mut reader = BufReader::new(stdout).lines();
        // Note: we can't stream to event_sink here because it requires &mut.
        // Collect all output, then report.
        while let Ok(Some(line)) = reader.next_line().await {
            output_lines.push(line);
        }
    }

    // Read stderr
    let mut stderr_lines = Vec::new();
    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            stderr_lines.push(line);
        }
    }

    // Wait for process with timeout (300 seconds)
    let timeout_duration = tokio::time::Duration::from_secs(300);
    let status = match tokio::time::timeout(timeout_duration, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => {
            return Ok(format!("Error waiting for command: {}", e));
        }
        Err(_) => {
            // Timeout — kill the process
            let _ = child.kill().await;
            return Ok("Error: Command execution timeout (300s)".to_string());
        }
    };

    // Build result
    let stdout_text = output_lines.join("\n");
    let stderr_text = stderr_lines.join("\n");
    let mut result = String::new();

    if status.success() {
        if stdout_text.is_empty() && stderr_text.is_empty() {
            result.push_str("Command succeeded (exit 0)");
        } else {
            result.push_str(&format!("Command succeeded (exit 0):\n{}", stdout_text));
            if !stderr_text.is_empty() {
                result.push_str(&format!("\n[stderr]: {}", stderr_text));
            }
        }
    } else {
        let code = status.code().unwrap_or(-1);
        let combined = if !stdout_text.is_empty() && !stderr_text.is_empty() {
            format!("{}\n[stderr]: {}", stdout_text, stderr_text)
        } else if !stderr_text.is_empty() {
            stderr_text
        } else {
            stdout_text
        };
        result.push_str(&format!("Command failed (exit {}):\n{}", code, combined));
    }

    Ok(result)
}

// ─── Phase 2: write_output ──────────────────────────────────────────────────

/// Execute `write_output`: write deliverable files to the output directory.
/// Ported from Python `builtin_tools.py` write_output implementation.
fn execute_write_output(args: &Value, workspace: &Path) -> Result<String> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .context("'file_path' is required")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;
    let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

    // Resolve output directory: SKILLLITE_OUTPUT_DIR > {workspace}/output
    let output_root = match types::get_output_dir() {
        Some(dir) => PathBuf::from(dir),
        None => workspace.join("output"),
    };

    // Resolve path within output_root
    let input = Path::new(file_path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        output_root.join(input)
    };

    let normalized = normalize_path(&resolved);
    if !normalized.starts_with(&output_root) {
        anyhow::bail!(
            "Path escapes output directory: {} (output_root: {})",
            file_path,
            output_root.display()
        );
    }

    // Create parent directories
    if let Some(parent) = normalized.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    if append {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&normalized)
            .with_context(|| format!("Failed to open output file for append: {}", normalized.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("Failed to append to output file: {}", normalized.display()))?;
    } else {
        std::fs::write(&normalized, content)
            .with_context(|| format!("Failed to write output file: {}", normalized.display()))?;
    }

    Ok(format!(
        "Successfully {} {} bytes to {}",
        if append { "appended" } else { "wrote" },
        content.len(),
        normalized.display()
    ))
}

// ─── Phase 3: chat_history, chat_plan (dedicated read tools) ─────────────────

/// Resolve chat data root (~/.skilllite/chat). Matches ChatSession layout.
fn chat_data_root() -> Result<PathBuf> {
    let root = crate::executor::workspace_root(None)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".skilllite"));
    Ok(root.join("chat"))
}

/// Normalize date string: "20260216" -> "2026-02-16".
fn normalize_date(date: &str) -> String {
    let s = date.trim().replace('-', "");
    if s.len() == 8 {
        format!("{}-{}-{}", &s[0..4], &s[4..6], &s[6..8])
    } else {
        date.to_string()
    }
}

/// Execute `chat_history`: read chat transcript for a session.
fn execute_chat_history(args: &Value) -> Result<String> {
    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let date: Option<String> = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(|s| normalize_date(s));

    let chat_root = chat_data_root()?;
    let transcripts_dir = chat_root.join("transcripts");

    if !transcripts_dir.exists() {
        return Ok("No chat history found. Transcripts directory does not exist.".to_string());
    }

    let entries = if let Some(ref d) = date {
        let path = crate::executor::transcript::transcript_path_for_session(
            &transcripts_dir,
            session_key,
            Some(d),
        );
        if path.exists() {
            crate::executor::transcript::read_entries(&path)?
        } else {
            return Ok(format!(
                "No chat history found for session '{}' on date {}.",
                session_key, d
            ));
        }
    } else {
        crate::executor::transcript::read_entries_for_session(&transcripts_dir, session_key)?
    };

    if entries.is_empty() {
        return Ok(format!(
            "No chat history found for session '{}'.",
            session_key
        ));
    }

    use crate::executor::transcript::TranscriptEntry;
    let mut lines = Vec::new();
    for entry in entries {
        match entry {
            TranscriptEntry::Session { .. } => {}
            TranscriptEntry::Message { role, content, .. } => {
                if let Some(c) = content {
                    lines.push(format!("[{}] {}", role, c.trim()));
                }
            }
            TranscriptEntry::Compaction { summary, .. } => {
                if let Some(s) = summary {
                    lines.push(format!("[compaction] {}", s));
                }
            }
            _ => {}
        }
    }
    Ok(lines.join("\n\n"))
}

/// Execute `chat_plan`: read task plan for a session.
fn execute_chat_plan(args: &Value) -> Result<String> {
    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let date_str = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(|s| normalize_date(s))
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let chat_root = chat_data_root()?;
    let plans_dir = chat_root.join("plans");
    let plan_path = plans_dir.join(format!("{}-{}.json", session_key, date_str));

    if !plan_path.exists() {
        return Ok(format!(
            "No plan found for session '{}' on date {}.",
            session_key, date_str
        ));
    }

    let content = std::fs::read_to_string(&plan_path)
        .with_context(|| format!("Failed to read plan: {}", plan_path.display()))?;
    let plan: Value = serde_json::from_str(&content)
        .with_context(|| "Invalid plan JSON")?;

    let task = plan.get("task").and_then(|v| v.as_str()).unwrap_or("");
    let empty: Vec<Value> = vec![];
    let steps = plan.get("steps").and_then(|v| v.as_array()).unwrap_or(&empty);

    let mut lines = vec![format!("Task: {}", task), "Steps:".to_string()];
    for (i, step) in steps.iter().enumerate() {
        let desc = step.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let status = step.get("status").and_then(|v| v.as_str()).unwrap_or("pending");
        lines.push(format!("  {}. [{}] {}", i + 1, status, desc));
    }
    Ok(lines.join("\n"))
}

/// Execute `list_output`: list files in the output directory (no path needed).
fn execute_list_output(args: &Value) -> Result<String> {
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let output_root = types::get_output_dir()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Output directory not configured (SKILLLITE_OUTPUT_DIR)"))?;

    if !output_root.exists() {
        return Ok("Output directory does not exist or is empty.".to_string());
    }
    if !output_root.is_dir() {
        anyhow::bail!("Output path is not a directory: {}", output_root.display());
    }

    let mut entries = Vec::new();
    list_dir_impl(&output_root, &output_root, recursive, &mut entries, 0)?;

    if entries.is_empty() {
        return Ok("Output directory is empty.".to_string());
    }

    Ok(entries.join("\n"))
}

// ─── Phase 2.5: preview_server ──────────────────────────────────────────────

use std::sync::Mutex;

/// Global state for active preview server (reuse / shutdown).
static ACTIVE_PREVIEW: Mutex<Option<PreviewServerState>> = Mutex::new(None);

struct PreviewServerState {
    serve_dir: String,
    port: u16,
}

/// Resolve a path within workspace OR the output directory.
/// preview_server needs to serve output files written by write_output,
/// which are in SKILLLITE_OUTPUT_DIR (typically ~/.skilllite/chat/output/).
fn resolve_within_workspace_or_output(path: &str, workspace: &Path) -> Result<PathBuf> {
    // First try workspace
    if let Ok(resolved) = resolve_within_workspace(path, workspace) {
        return Ok(resolved);
    }

    // Then try output directory (where write_output saves files)
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

/// Execute `preview_server`: start a local HTTP file server in a daemon thread.
/// Ported from Python `_start_preview_server` in builtin_tools.py.
///
/// Features:
/// - Binds to 127.0.0.1 only
/// - Auto-scans ports (default 8765, tries +19)
/// - Reuses server if same directory
/// - Shuts down old server if different directory
/// - Sends no-cache headers
/// - Opens browser via `open` / `xdg-open`
fn execute_preview_server(args: &Value, workspace: &Path) -> Result<String> {
    let dir_path = get_path_arg(args, true)
        .ok_or_else(|| anyhow::anyhow!("'directory_path' or 'path' is required"))?;
    let requested_port = args
        .get("port")
        .and_then(|v| v.as_u64())
        .unwrap_or(8765) as u16;
    let should_open_browser = args
        .get("open_browser")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Allow both workspace and output directory (where write_output saves files)
    let resolved = resolve_within_workspace_or_output(&dir_path, workspace)?;

    // If a file path was given, serve its parent directory
    let (serve_dir, target_file) = if resolved.is_file() {
        let fname = resolved.file_name().map(|f| f.to_string_lossy().to_string());
        (resolved.parent().unwrap_or(&resolved).to_path_buf(), fname)
    } else {
        (resolved.clone(), None)
    };

    if !serve_dir.exists() {
        anyhow::bail!("Path not found: {}", dir_path);
    }

    let serve_dir_str = serve_dir.to_string_lossy().to_string();

    // Check if server already running for this directory
    {
        let guard = ACTIVE_PREVIEW.lock().unwrap();
        if let Some(ref state) = *guard {
            if state.serve_dir == serve_dir_str {
                let url = build_preview_url(state.port, target_file.as_deref());
                if should_open_browser {
                    open_browser(&url);
                }
                return Ok(format!(
                    "Preview server already running at {}\n\n\
                     Open in browser: {}\n\
                     Serving directory: {}\n\
                     (Browser opened with fresh page.)",
                    url, url, serve_dir_str
                ));
            }
        }
    }

    // Try to bind to a port
    let listener = {
        let mut bound = None;
        for p in requested_port..requested_port.saturating_add(20).min(65535) {
            match std::net::TcpListener::bind(("127.0.0.1", p)) {
                Ok(l) => {
                    bound = Some((l, p));
                    break;
                }
                Err(_) => continue,
            }
        }
        bound
    };

    let (listener, used_port) = match listener {
        Some((l, p)) => (l, p),
        None => anyhow::bail!(
            "Could not bind to port {} (tried {}-{})",
            requested_port,
            requested_port,
            requested_port + 19
        ),
    };

    // Update global state
    {
        let mut guard = ACTIVE_PREVIEW.lock().unwrap();
        *guard = Some(PreviewServerState {
            serve_dir: serve_dir_str.clone(),
            port: used_port,
        });
    }

    // Spawn daemon thread for the HTTP file server
    let serve_dir_clone = serve_dir.clone();
    std::thread::Builder::new()
        .name("preview-server".to_string())
        .spawn(move || {
            run_file_server(listener, &serve_dir_clone);
        })
        .context("Failed to spawn preview server thread")?;

    let url = build_preview_url(used_port, target_file.as_deref());
    if should_open_browser {
        open_browser(&url);
    }

    Ok(format!(
        "Preview server started at {}\n\n\
         Open in browser: {}\n\
         Serving directory: {}\n\
         (Server runs in background. Stops when you exit.)",
        url, url, serve_dir_str
    ))
}

/// Build the preview URL with optional filename.
fn build_preview_url(port: u16, filename: Option<&str>) -> String {
    match filename {
        Some(f) => format!("http://127.0.0.1:{}/{}", port, f),
        None => format!("http://127.0.0.1:{}", port),
    }
}

/// Open a URL in the default browser.
fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
    } else {
        Ok(std::process::Command::new("true").spawn().unwrap())
    };
}

/// Minimal HTTP file server loop. Handles GET requests, serves static files
/// with no-cache headers. Runs in a daemon thread.
fn run_file_server(listener: std::net::TcpListener, serve_dir: &Path) {
    use std::io::{BufRead, BufReader, Write};

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let reader = BufReader::new(&stream);
        let request_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            _ => continue,
        };

        // Parse "GET /path HTTP/1.x"
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] != "GET" {
            let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n");
            continue;
        }

        let request_path = parts[1];
        // Strip query string
        let clean_path = request_path.split('?').next().unwrap_or("/");
        // URL decode %XX sequences (basic)
        let decoded = url_decode(clean_path);
        let rel = decoded.trim_start_matches('/');
        let is_root_request = rel.is_empty();

        // Root: always serve fresh directory listing (never default to index.html).
        // This ensures newly written files are visible instead of stale index from previous sessions.
        if is_root_request {
            serve_directory_fallback(&mut stream, serve_dir);
            continue;
        }

        let file_path = serve_dir.join(rel);
        let normalized = normalize_path(&file_path);

        // Security: path must stay within serve_dir
        if !normalized.starts_with(serve_dir) {
            let body = "403 Forbidden";
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            continue;
        }

        if normalized.is_file() {
            match std::fs::read(&normalized) {
                Ok(content) => {
                    let mime = guess_mime(&normalized);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: {}\r\n\
                         Content-Length: {}\r\n\
                         Cache-Control: no-store, no-cache, must-revalidate, max-age=0\r\n\
                         Pragma: no-cache\r\n\
                         Connection: close\r\n\r\n",
                        mime,
                        content.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.write_all(&content);
                }
                Err(_) => {
                    let body = "500 Internal Server Error";
                    let resp = format!(
                        "HTTP/1.1 500 Internal Server Error\r\n\
                         Content-Length: {}\r\n\
                         Connection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        } else {
            let body = "404 Not Found";
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}

/// Fallback for root URL: redirect to the most recently modified HTML file
/// so users see the latest result directly instead of a file list.
fn serve_directory_fallback(stream: &mut std::net::TcpStream, serve_dir: &Path) {
    use std::io::Write;

    // Collect HTML files with mtime (newest first)
    let mut html_with_mtime: Vec<(String, std::time::SystemTime)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(serve_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "html" || ext == "htm" {
                        if let (Some(name), Ok(meta)) = (
                            path.file_name().and_then(|n| n.to_str()),
                            path.metadata(),
                        ) {
                            if let Ok(mtime) = meta.modified() {
                                html_with_mtime.push((name.to_string(), mtime));
                            }
                        }
                    }
                }
            }
        }
    }

    if !html_with_mtime.is_empty() {
        // Sort by mtime descending (newest first), then by name for ties
        html_with_mtime.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let newest = &html_with_mtime[0].0;
        let redirect_url = format!("/{}", newest);
        let resp = format!(
            "HTTP/1.1 302 Found\r\n\
             Location: {}\r\n\
             Content-Length: 0\r\n\
             Connection: close\r\n\r\n",
            redirect_url
        );
        let _ = stream.write_all(resp.as_bytes());
    } else {
        // No HTML files — list all files
        let mut all_files: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(serve_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') {
                            all_files.push(name.to_string());
                        }
                    }
                }
            }
        }
        all_files.sort();

        let body = generate_listing_html("Files", &all_files);
        let resp = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Content-Length: {}\r\n\
             Cache-Control: no-store\r\n\
             Connection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
    }
}

fn generate_listing_html(title: &str, files: &[String]) -> String {
    let items: Vec<String> = files
        .iter()
        .map(|f| format!("<li><a href=\"/{}\">{}</a></li>", f, f))
        .collect();
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\">\
         <title>SkillLite Preview - {}</title>\
         <style>body{{font-family:system-ui,-apple-system,sans-serif;max-width:600px;margin:40px auto;padding:0 20px}}\
         a{{color:#2563eb;text-decoration:none;font-size:18px}}a:hover{{text-decoration:underline}}\
         li{{margin:8px 0}}h1{{color:#1e293b}}</style></head>\
         <body><h1>{}</h1><ul>{}</ul></body></html>",
        title, title, items.join("")
    )
}

/// Basic URL decoding for %XX sequences.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| hex_val(c));
            let lo = chars.next().and_then(|c| hex_val(c));
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Guess MIME type from file extension.
fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

// ─── Long content handling ──────────────────────────────────────────────────

/// Process tool result content: truncate if too long.
///
/// This is the **synchronous** fast path. Returns `Some(truncated)` if the
/// content was handled (either unchanged or truncated). Returns `None` if the
/// content exceeds the summarization threshold and should be handled by the
/// async `long_text::summarize_long_content` in `agent_loop`.
///
/// Ported from Python `_process_tool_result_content` with the addition of
/// env-configurable thresholds (Phase 2).
pub fn process_tool_result_content(content: &str) -> Option<String> {
    let max_chars = types::get_tool_result_max_chars();
    let summarize_threshold = types::get_summarize_threshold();
    let len = content.len();

    if len <= max_chars {
        return Some(content.to_string());
    }

    if len > summarize_threshold {
        // Signal caller to use async LLM summarization
        return None;
    }

    // Between max_chars and summarize_threshold: simple truncation (UTF-8 safe)
    Some(format!(
        "{}\n\n[... 结果已截断，原文共 {} 字符，仅保留前 {} 字符 ...]",
        safe_truncate(content, max_chars),
        len,
        max_chars
    ))
}

/// Synchronous fallback: head+tail truncation for content that exceeds the
/// summarize threshold but where LLM summarization is not available or failed.
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
            "old_string": "hello",
            "new_string": "hi",
            "replace_all": false
        });
        let result = execute_builtin_tool("search_replace", &args.to_string(), workspace);
        assert!(!result.is_error);
        assert!(result.content.contains("Successfully replaced 1 occurrence"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi world\nhello again\n");
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
    fn test_search_replace_output_directory() {
        // output/ inside workspace: path "output/index.html" resolves to workspace/output/index.html
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
}
