//! File operations: read_file, write_file, search_replace, list_directory, file_exists.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{ToolDefinition, FunctionDef};

use super::{
    get_path_arg, is_sensitive_write_path, list_dir_impl,
    resolve_within_workspace, resolve_within_workspace_or_output,
};

// ─── Tool definitions ───────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
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
                description: "Replace exact text in a file. Use for precise edits instead of read_file + write_file. Supports workspace and output directory (path relative to workspace or output dir). old_string must match exactly (including whitespace). Use normalize_whitespace: true to allow optional trailing whitespace after old_string (reduces match failure). If old_string appears multiple times, use replace_all: true to replace all, or false (default) to replace only the first occurrence.".to_string(),
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
                        },
                        "normalize_whitespace": {
                            "type": "boolean",
                            "description": "If true, allow optional trailing whitespace after old_string when matching (reduces failure from minor whitespace differences). Default: false."
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
    ]
}

// ─── Execution ──────────────────────────────────────────────────────────────

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
    let normalize_whitespace = args
        .get("normalize_whitespace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_sensitive_write_path(&path_str) {
        anyhow::bail!(
            "Blocked: editing sensitive file '{}' is not allowed",
            path_str
        );
    }

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    let content = std::fs::read_to_string(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;

    let (count, new_content) = if old_string.is_empty() {
        anyhow::bail!("old_string cannot be empty");
    } else if normalize_whitespace {
        let escaped = regex::escape(old_string);
        let pattern = format!(r"({})\s*", escaped);
        let re = regex::Regex::new(&pattern)
            .map_err(|e| anyhow::anyhow!("Invalid old_string for regex: {}", e))?;
        let count = re.find_iter(&content).count();
        if count == 0 {
            anyhow::bail!(
                "old_string not found in file (with normalize_whitespace). Ensure it matches (trailing whitespace is ignored)."
            );
        }
        let repl = regex::NoExpand(new_string);
        let new_content = if replace_all {
            re.replace_all(&content, repl).into_owned()
        } else {
            re.replacen(&content, 1, repl).into_owned()
        };
        (count, new_content)
    } else {
        let count = if replace_all {
            content.matches(old_string).count()
        } else {
            if content.contains(old_string) { 1 } else { 0 }
        };
        if count == 0 {
            anyhow::bail!(
                "old_string not found in file. Ensure it matches exactly (including whitespace and newlines). Use normalize_whitespace: true to allow trailing whitespace."
            );
        }
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        (count, new_content)
    };

    std::fs::write(&resolved, &new_content)
        .with_context(|| format!("Failed to write file: {}", path_str))?;

    Ok(format!(
        "Successfully replaced {} occurrence(s) in {}",
        count, path_str
    ))
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
