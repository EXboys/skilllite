//! File operations: read_file, write_file, search_replace, insert_lines, grep_files, list_directory, file_exists.
//!
//! Split into submodules:
//! - `search_replace`: search_replace, preview_edit, insert_lines + fuzzy matching + backup + validation
//! - `grep`: grep_files

mod search_replace;
mod grep;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{ToolDefinition, FunctionDef};

use super::{get_path_arg, is_sensitive_write_path, list_dir_impl, resolve_within_workspace, resolve_within_workspace_or_output};

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "read_file".to_string(),
                description: "Read the contents of a file. Returns UTF-8 text with line numbers (N|line). Use start_line/end_line for partial reads to save context.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Start line number (1-based, inclusive). Omit to read from beginning."
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "End line number (1-based, inclusive). Omit to read to end."
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
                description: "Replace text in a file with automatic fuzzy matching. Tries exact match first, then falls back to whitespace-insensitive and similarity-based matching. Use dry_run: true to preview without writing.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Text to find (fuzzy matching handles minor whitespace differences automatically)"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Text to replace old_string with"
                        },
                        "replace_all": {
                            "type": "boolean",
                            "description": "If true, replace all occurrences. Default: false (replace first only)."
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "If true, preview the edit without writing to disk. Default: false."
                        }
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "insert_lines".to_string(),
                description: "Insert content after a specific line number. Use line=0 to insert at the beginning of the file.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (relative to workspace or absolute)"
                        },
                        "line": {
                            "type": "integer",
                            "description": "Insert after this line number (0 = beginning of file, 1 = after first line)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to insert"
                        }
                    },
                    "required": ["path", "line", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "grep_files".to_string(),
                description: "Search file contents using regex. Returns file:line:content matches. Auto-skips .git, node_modules, target, and binary files.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory to search in (relative to workspace). Default: workspace root."
                        },
                        "include": {
                            "type": "string",
                            "description": "File type filter (e.g. '*.rs', '*.py'). Default: all text files."
                        }
                    },
                    "required": ["pattern"]
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
    ]
}


pub(super) fn execute_read_file(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    let start_line = args.get("start_line").and_then(|v| v.as_u64()).map(|v| v as usize);
    let end_line = args.get("end_line").and_then(|v| v.as_u64()).map(|v| v as usize);

    match std::fs::read_to_string(&resolved) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();

            let start = start_line.unwrap_or(1).max(1);
            let end = end_line.unwrap_or(total).min(total);

            if start > total {
                return Ok(format!("[File has {} lines, requested start_line={}]", total, start));
            }
            if start > end {
                return Ok(format!("[Invalid range: start_line={} > end_line={}]", start, end));
            }

            let mut output = String::new();
            for i in (start - 1)..end {
                output.push_str(&format!("{:>6}|{}\n", i + 1, lines[i]));
            }

            if start_line.is_some() || end_line.is_some() {
                output.push_str(&format!("\n[Showing lines {}-{} of {} total]", start, end, total));
            }

            Ok(output)
        }
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

pub(super) fn execute_write_file(args: &Value, workspace: &Path) -> Result<String> {
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

pub(super) fn execute_search_replace(args: &Value, workspace: &Path) -> Result<String> {
    search_replace::execute_search_replace(args, workspace)
}

pub(super) fn execute_preview_edit(args: &Value, workspace: &Path) -> Result<String> {
    search_replace::execute_preview_edit(args, workspace)
}

pub(super) fn execute_insert_lines(args: &Value, workspace: &Path) -> Result<String> {
    search_replace::execute_insert_lines(args, workspace)
}

pub(super) fn execute_grep_files(args: &Value, workspace: &Path) -> Result<String> {
    grep::execute_grep_files(args, workspace)
}

pub(super) fn execute_list_directory(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, true).unwrap_or_else(|| ".".to_string());
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

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

pub(super) fn execute_file_exists(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' or 'file_path' is required"))?;

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
