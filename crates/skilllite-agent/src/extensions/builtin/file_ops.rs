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
                name: "preview_edit".to_string(),
                description: "Preview a search_replace edit without writing to disk. Returns a structured diff summary (changed/occurrences/first_changed_line/diff_excerpt). Use this for high-risk edits before applying search_replace.".to_string(),
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
                            "description": "If true, replace all occurrences. Default: false (requires unique match)."
                        },
                        "normalize_whitespace": {
                            "type": "boolean",
                            "description": "If true, allow trailing spaces/tabs before newline when matching old_string. Default: false."
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
    execute_replace_like(args, workspace, true)
}

pub(super) fn execute_preview_edit(args: &Value, workspace: &Path) -> Result<String> {
    execute_replace_like(args, workspace, false)
}

fn execute_replace_like(args: &Value, workspace: &Path, apply_changes: bool) -> Result<String> {
    let tool_name = if apply_changes { "search_replace" } else { "preview_edit" };
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
        skilllite_core::observability::audit_edit_failed(
            &path_str, tool_name, "sensitive_path_blocked",
        );
        anyhow::bail!(
            "Blocked: editing sensitive file '{}' is not allowed",
            path_str
        );
    }

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;

    if !resolved.exists() {
        skilllite_core::observability::audit_edit_failed(&path_str, tool_name, "file_not_found");
        anyhow::bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        skilllite_core::observability::audit_edit_failed(&path_str, tool_name, "path_is_directory");
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    let content = std::fs::read_to_string(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;

    let match_result: Result<(usize, usize, usize, usize, String)> = if old_string.is_empty() {
        Err(anyhow::anyhow!("old_string cannot be empty"))
    } else if normalize_whitespace {
        let escaped = regex::escape(old_string);
        let pattern = format!(r"({})([ \t]*)(\r?\n|$)", escaped);
        let re = regex::Regex::new(&pattern)
            .map_err(|e| anyhow::anyhow!("Invalid old_string for regex: {}", e))?;
        let matches: Vec<_> = re.find_iter(&content).collect();
        let count = matches.len();
        if count == 0 {
            Err(anyhow::anyhow!(
                "old_string not found in file (with normalize_whitespace). Ensure it matches (trailing whitespace is ignored)."
            ))
        } else if !replace_all && count > 1 {
            Err(anyhow::anyhow!(
                "Found {} occurrences of old_string in {}. search_replace requires a unique match by default; add more context to old_string or set replace_all=true.",
                count, path_str
            ))
        } else {
            let first_caps = re
                .captures(&content)
                .ok_or_else(|| anyhow::anyhow!("Failed to capture first match for {}", path_str))?;
            let first_match = first_caps
                .get(0)
                .ok_or_else(|| anyhow::anyhow!("Failed to capture first full match for {}", path_str))?;
            let new_content = if replace_all {
                re.replace_all(&content, |_caps: &regex::Captures| {
                    let newline = _caps.get(3).map_or("", |m| m.as_str());
                    format!("{}{}", new_string, newline)
                })
                .into_owned()
            } else {
                re.replacen(&content, 1, |_caps: &regex::Captures| {
                    let newline = _caps.get(3).map_or("", |m| m.as_str());
                    format!("{}{}", new_string, newline)
                })
                .into_owned()
            };
            Ok((
                count,
                if replace_all { count } else { 1 },
                first_match.start(),
                first_match.end() - first_match.start(),
                new_content,
            ))
        }
    } else {
        let count = content.matches(old_string).count();
        if count == 0 {
            Err(anyhow::anyhow!(
                "old_string not found in file. Ensure it matches exactly (including whitespace and newlines). Use normalize_whitespace: true to allow trailing whitespace."
            ))
        } else if !replace_all && count > 1 {
            Err(anyhow::anyhow!(
                "Found {} occurrences of old_string in {}. search_replace requires a unique match by default; add more context to old_string or set replace_all=true.",
                count, path_str
            ))
        } else {
            let first_match_start = content.find(old_string).unwrap_or(0);
            let new_content = if replace_all {
                content.replace(old_string, new_string)
            } else {
                content.replacen(old_string, new_string, 1)
            };
            Ok((
                count,
                if replace_all { count } else { 1 },
                first_match_start,
                old_string.len(),
                new_content,
            ))
        }
    };

    let (total_occurrences, replaced_occurrences, first_match_start, first_match_len, new_content) =
        match match_result {
            Ok(v) => v,
            Err(e) => {
                skilllite_core::observability::audit_edit_failed(
                    &path_str,
                    tool_name,
                    &e.to_string(),
                );
                return Err(e);
            }
        };

    if content == new_content {
        skilllite_core::observability::audit_edit_failed(
            &path_str, tool_name, "no_change_produced",
        );
        anyhow::bail!("No changes were made: replacement produced identical content");
    }

    let first_changed_line = content[..first_match_start].bytes().filter(|b| *b == b'\n').count() + 1;
    let old_excerpt = safe_excerpt(&content, first_match_start, first_match_len, 200);
    let new_excerpt = safe_excerpt(&new_content, first_match_start, new_string.len(), 200);
    let diff_excerpt = format!("- {}\n+ {}", old_excerpt, new_excerpt);

    if apply_changes {
        std::fs::write(&resolved, &new_content)
            .with_context(|| format!("Failed to write file: {}", path_str))?;
        skilllite_core::observability::audit_edit_applied(
            &path_str, replaced_occurrences, first_changed_line, &diff_excerpt,
        );
    } else {
        skilllite_core::observability::audit_edit_previewed(
            &path_str, replaced_occurrences, first_changed_line, &diff_excerpt,
        );
    }

    let result = json!({
        "path": path_str,
        "changed": true,
        "occurrences": replaced_occurrences,
        "total_occurrences": total_occurrences,
        "first_changed_line": first_changed_line,
        "diff_excerpt": diff_excerpt
    });

    if apply_changes {
        Ok(format!(
            "Successfully replaced {} occurrence(s) in {}\n{}",
            replaced_occurrences,
            path_str,
            serde_json::to_string_pretty(&result)?
        ))
    } else {
        Ok(format!(
            "Preview edit for {} (no changes written)\n{}",
            path_str,
            serde_json::to_string_pretty(&result)?
        ))
    }
}

fn safe_excerpt(content: &str, start: usize, span_len: usize, max_len: usize) -> String {
    let prefix = 80usize;
    let suffix = 80usize;
    let begin = floor_char_boundary(content, start.saturating_sub(prefix));
    let end = ceil_char_boundary(content, (start + span_len + suffix).min(content.len()));
    let mut excerpt = content[begin..end].replace('\n', "\\n");
    if excerpt.len() > max_len {
        excerpt.truncate(max_len);
        excerpt.push_str("...");
    }
    excerpt
}

fn floor_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
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
