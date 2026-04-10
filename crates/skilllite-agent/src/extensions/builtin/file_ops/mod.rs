//! File operations: read_file, write_file, search_replace, insert_lines, grep_files, list_directory, file_exists.
//!
//! Split into submodules:
//! - `search_replace`: search_replace, preview_edit, insert_lines + fuzzy matching + backup + validation
//! - `grep`: grep_files

mod grep;
mod search_replace;

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{EventSink, FunctionDef, ToolDefinition};

use super::{
    filter_sensitive_content_in_text, get_path_arg, is_key_write_path, is_sensitive_read_path,
    is_sensitive_write_path, resolve_within_workspace, resolve_within_workspace_or_output,
};
use crate::high_risk;

/// Block a common LLM mistake: `write_file` with `users/<x>/output/...` under the workspace.
/// Those files are not under the real output dir (`SKILLLITE_OUTPUT_DIR` / `<workspace>/output`),
/// so the desktop **Output** panel lists nothing. Deliverables must use `write_output`.
fn reject_misplaced_output_style_write_file_path(path_str: &str) -> Result<()> {
    let normalized = path_str.replace('\\', "/");
    let trimmed = normalized.trim().trim_start_matches("./");
    if trimmed.starts_with('/') {
        return Ok(());
    }
    let parts: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 4
        && parts[0].eq_ignore_ascii_case("users")
        && parts[2].eq_ignore_ascii_case("output")
    {
        bail!(
            "Blocked path '{}': generated deliverables must use **write_output** (file_path = filename only, or a path relative to the output directory) so they appear in the app Output panel. \
             Use **write_file** only for normal project source files (e.g. src/, docs/ when editing the repo). \
             Do not invent users/.../output/... under the workspace.",
            path_str
        );
    }
    Ok(())
}

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "read_file".to_string(),
                description: "Read the contents of a file. Returns UTF-8 text with line numbers (N|line). Use start_line/end_line for partial reads to save context. Blocks .env, .key, .git/config. Other files have sensitive values (API_KEY, password, etc.) redacted.".to_string(),
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
                description: "Write content to a file under the workspace (source, config, docs you are editing). Creates parent directories if needed. Blocks sensitive paths (.env, .key, .git/config). \
For **deliverables** the user should open from the app (reports, tutorials, exports, generated markdown/HTML, screenshots paths in configs): you MUST use **write_output**, not write_file — otherwise files are easy to \"lose\" (not listed in the Output panel). \
With write_file, use normal project-relative paths (e.g. src/, docs/); do not invent nested paths like users/.../output/.... Use append: true to append instead of overwriting.".to_string(),
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
                description: "List files and directories as an ASCII tree (├──/└──). Supports recursive listing; skips heavy dirs (node_modules, .git, target, etc.).".to_string(),
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
        .ok_or_else(|| crate::Error::validation("'path' or 'file_path' is required"))?;

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        bail!("Path is a directory, not a file: {}", path_str);
    }

    // A11: .env、.key、.git/config 等配置和密码文件直接拒绝
    if is_sensitive_read_path(&path_str) {
        bail!(
            "Blocked: reading sensitive file '{}' (.env, .key, .git/config, etc.) is not allowed",
            path_str
        );
    }

    let start_line = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let end_line = args
        .get("end_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    match skilllite_fs::read_file(&resolved) {
        Ok(content) => {
            let (content, was_redacted) = filter_sensitive_content_in_text(&content);
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();

            let start = start_line.unwrap_or(1).max(1);
            let end = end_line.unwrap_or(total).min(total);

            if start > total {
                return Ok(format!(
                    "[File has {} lines, requested start_line={}]",
                    total, start
                ));
            }
            if start > end {
                return Ok(format!(
                    "[Invalid range: start_line={} > end_line={}]",
                    start, end
                ));
            }

            let mut output = String::new();
            for (i, line) in lines.iter().enumerate().take(end).skip(start - 1) {
                output.push_str(&format!("{:>6}|{}\n", i + 1, line));
            }

            if start_line.is_some() || end_line.is_some() {
                output.push_str(&format!(
                    "\n[Showing lines {}-{} of {} total]",
                    start, end, total
                ));
            }

            if was_redacted {
                output.push_str(
                    "\n\n[⚠️ Sensitive values (API_KEY, PASSWORD, etc.) have been redacted]",
                );
            }

            Ok(output)
        }
        Err(e) => {
            let is_binary = match &e {
                skilllite_fs::Error::Io(io_err) => io_err.kind() == std::io::ErrorKind::InvalidData,
                skilllite_fs::Error::Other(anyhow_err) => anyhow_err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|ie| ie.kind() == std::io::ErrorKind::InvalidData),
                _ => false,
            };
            if is_binary {
                let size = match skilllite_fs::file_exists(&resolved)? {
                    skilllite_fs::PathKind::File(len) => len,
                    _ => 0,
                };
                Ok(format!(
                    "[Binary file, {} bytes. Cannot display as text.]",
                    size
                ))
            } else {
                Err(e.into())
            }
        }
    }
}

pub(super) fn execute_write_file(
    args: &Value,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| crate::Error::validation("'path' or 'file_path' is required"))?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;
    let append = args
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_sensitive_write_path(&path_str) {
        bail!(
            "Blocked: writing to sensitive file '{}' is not allowed",
            path_str
        );
    }

    reject_misplaced_output_style_write_file_path(&path_str)?;

    // A11: 关键路径确认
    if high_risk::confirm_write_key_path() && is_key_write_path(&path_str) {
        if let Some(sink) = event_sink {
            let preview = content.chars().take(200).collect::<String>();
            let suffix = if content.len() > 200 { "..." } else { "" };
            let msg = format!(
                "⚠️ 关键路径写入确认\n\n路径: {}\n内容预览 (前200字符):\n{}\n{}\n\n确认写入?",
                path_str, preview, suffix
            );
            if !sink.on_confirmation_request(&crate::types::ConfirmationRequest::new(
                msg,
                crate::types::RiskTier::ConfirmRequired,
            )) {
                return Ok("User cancelled: write to key path not confirmed".to_string());
            }
        }
    }

    let resolved = resolve_within_workspace(&path_str, workspace)?;

    if append {
        skilllite_fs::append_file(&resolved, content)
            .with_context(|| format!("Failed to append to file: {}", path_str))?;
    } else {
        skilllite_fs::write_file(&resolved, content)
            .with_context(|| format!("Failed to write file: {}", path_str))?;
    }

    Ok(format!(
        "Successfully {} {} bytes to {}",
        if append { "appended" } else { "wrote" },
        content.len(),
        path_str
    ))
}

pub(super) fn execute_search_replace(
    args: &Value,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
    search_replace::execute_search_replace(args, workspace, event_sink)
}

pub(super) fn execute_preview_edit(args: &Value, workspace: &Path) -> Result<String> {
    search_replace::execute_preview_edit(args, workspace)
}

pub(super) fn execute_insert_lines(
    args: &Value,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
    search_replace::execute_insert_lines(args, workspace, event_sink)
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
    Ok(skilllite_fs::directory_tree(&resolved, recursive)?)
}

pub(super) fn execute_file_exists(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| crate::Error::validation("'path' or 'file_path' is required"))?;

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;
    match skilllite_fs::file_exists(&resolved)? {
        skilllite_fs::PathKind::NotFound => Ok(format!("{}: does not exist", path_str)),
        skilllite_fs::PathKind::Dir => Ok(format!("{}: directory", path_str)),
        skilllite_fs::PathKind::File(size) => Ok(format!("{}: file ({} bytes)", path_str, size)),
    }
}

#[cfg(test)]
mod misplaced_output_path_tests {
    use super::reject_misplaced_output_style_write_file_path;

    #[test]
    fn blocks_users_segment_output_pattern() {
        assert!(
            reject_misplaced_output_style_write_file_path("users/z/output/tutorial.md").is_err()
        );
    }

    #[test]
    fn allows_output_root_relative() {
        assert!(reject_misplaced_output_style_write_file_path("output/note.md").is_ok());
    }

    #[test]
    fn allows_src_paths() {
        assert!(reject_misplaced_output_style_write_file_path("src/lib.rs").is_ok());
    }
}
