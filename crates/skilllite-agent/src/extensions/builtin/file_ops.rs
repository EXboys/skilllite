//! File operations: read_file, write_file, search_replace, insert_lines, list_directory, file_exists.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{ToolDefinition, FunctionDef};

use super::{
    get_path_arg, is_sensitive_write_path, list_dir_impl,
    resolve_within_workspace, resolve_within_workspace_or_output,
};

const FUZZY_THRESHOLD: f64 = 0.85;

// ─── Tool definitions ───────────────────────────────────────────────────────

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
    execute_replace_like(args, workspace, true)
}

/// Backward-compatible handler: preview_edit is now search_replace with dry_run=true.
pub(super) fn execute_preview_edit(args: &Value, workspace: &Path) -> Result<String> {
    execute_replace_like(args, workspace, false)
}

pub(super) fn execute_insert_lines(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = get_path_arg(args, false)
        .ok_or_else(|| anyhow::anyhow!("'path' is required"))?;
    let line_num = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("'line' is required (0 = beginning of file)"))? as usize;
    let insert_content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;

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

    let content = std::fs::read_to_string(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    if line_num > total {
        anyhow::bail!(
            "Line {} is beyond end of file ({} lines)",
            line_num,
            total
        );
    }

    let offsets = line_byte_offsets(&content);
    let insert_at = if line_num == 0 {
        0
    } else {
        offsets.get(line_num).copied().unwrap_or(content.len())
    };

    let needs_preceding_newline =
        line_num > 0 && insert_at == content.len() && !content.is_empty() && !content.ends_with('\n');

    let insert_with_newline = if insert_content.ends_with('\n') {
        insert_content.to_string()
    } else {
        format!("{}\n", insert_content)
    };

    let new_content = if needs_preceding_newline {
        format!(
            "{}\n{}{}",
            &content[..insert_at],
            insert_with_newline,
            &content[insert_at..]
        )
    } else {
        format!(
            "{}{}{}",
            &content[..insert_at],
            insert_with_newline,
            &content[insert_at..]
        )
    };

    std::fs::write(&resolved, &new_content)
        .with_context(|| format!("Failed to write file: {}", path_str))?;

    let inserted_lines = insert_content.lines().count().max(1);
    let result = json!({
        "path": path_str,
        "inserted_after_line": line_num,
        "lines_inserted": inserted_lines,
        "new_total_lines": total + inserted_lines
    });

    Ok(format!(
        "Successfully inserted {} line(s) after line {} in {}\n{}",
        inserted_lines,
        line_num,
        path_str,
        serde_json::to_string_pretty(&result)?
    ))
}

fn execute_replace_like(args: &Value, workspace: &Path, apply_changes: bool) -> Result<String> {
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let should_write = apply_changes && !dry_run;
    let tool_name = if should_write {
        "search_replace"
    } else {
        "preview_edit"
    };

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
    let replace_all = args
        .get("replace_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    // Backward compat: still honoured when explicitly passed
    let normalize_whitespace = args
        .get("normalize_whitespace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_sensitive_write_path(&path_str) {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            "sensitive_path_blocked",
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

    // (match_type, total_occurrences, replaced_count, first_match_start, first_match_len, new_content)
    type MatchTuple = (String, usize, usize, usize, usize, String);

    let match_result: Result<MatchTuple> = if old_string.is_empty() {
        Err(anyhow::anyhow!("old_string cannot be empty"))
    } else if normalize_whitespace {
        // Legacy regex-based trailing-whitespace matching (backward compat)
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
                .ok_or_else(|| {
                    anyhow::anyhow!("Failed to capture first full match for {}", path_str)
                })?;
            let new_content = if replace_all {
                re.replace_all(&content, |caps: &regex::Captures| {
                    let newline = caps.get(3).map_or("", |m| m.as_str());
                    format!("{}{}", new_string, newline)
                })
                .into_owned()
            } else {
                re.replacen(&content, 1, |caps: &regex::Captures| {
                    let newline = caps.get(3).map_or("", |m| m.as_str());
                    format!("{}{}", new_string, newline)
                })
                .into_owned()
            };
            Ok((
                "exact".to_string(),
                count,
                if replace_all { count } else { 1 },
                first_match.start(),
                first_match.end() - first_match.start(),
                new_content,
            ))
        }
    } else {
        // Standard path with fuzzy fallback
        let exact_count = content.matches(old_string).count();
        if exact_count > 0 {
            if !replace_all && exact_count > 1 {
                Err(anyhow::anyhow!(
                    "Found {} occurrences of old_string in {}. search_replace requires a unique match by default; add more context to old_string or set replace_all=true.",
                    exact_count, path_str
                ))
            } else {
                let first_start = content.find(old_string).unwrap_or(0);
                let new_content = if replace_all {
                    content.replace(old_string, new_string)
                } else {
                    content.replacen(old_string, new_string, 1)
                };
                Ok((
                    "exact".to_string(),
                    exact_count,
                    if replace_all { exact_count } else { 1 },
                    first_start,
                    old_string.len(),
                    new_content,
                ))
            }
        } else if !replace_all {
            // Fuzzy fallback (single replacement only)
            match fuzzy_find(&content, old_string) {
                Some(fm) => {
                    let new_content = format!(
                        "{}{}{}",
                        &content[..fm.start],
                        new_string,
                        &content[fm.end..],
                    );
                    Ok((fm.match_type, 1, 1, fm.start, fm.end - fm.start, new_content))
                }
                None => Err(anyhow::anyhow!(
                    "old_string not found in file (tried exact + fuzzy matching). Ensure old_string matches the file content. Use read_file with line numbers to verify the exact text."
                )),
            }
        } else {
            Err(anyhow::anyhow!(
                "old_string not found in file. Ensure it matches exactly (including whitespace and newlines)."
            ))
        }
    };

    let (match_type, total_occurrences, replaced_occurrences, first_match_start, first_match_len, new_content) =
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
            &path_str,
            tool_name,
            "no_change_produced",
        );
        anyhow::bail!("No changes were made: replacement produced identical content");
    }

    let first_changed_line = content[..first_match_start]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1;
    let old_excerpt = safe_excerpt(&content, first_match_start, first_match_len, 200);
    let new_excerpt = safe_excerpt(&new_content, first_match_start, new_string.len(), 200);
    let diff_excerpt = format!("- {}\n+ {}", old_excerpt, new_excerpt);

    if should_write {
        std::fs::write(&resolved, &new_content)
            .with_context(|| format!("Failed to write file: {}", path_str))?;
        skilllite_core::observability::audit_edit_applied(
            &path_str,
            replaced_occurrences,
            first_changed_line,
            &diff_excerpt,
        );
    } else {
        skilllite_core::observability::audit_edit_previewed(
            &path_str,
            replaced_occurrences,
            first_changed_line,
            &diff_excerpt,
        );
    }

    let result = json!({
        "path": path_str,
        "changed": true,
        "match_type": match_type,
        "occurrences": replaced_occurrences,
        "total_occurrences": total_occurrences,
        "first_changed_line": first_changed_line,
        "diff_excerpt": diff_excerpt
    });

    if should_write {
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

// ─── Fuzzy matching (internal, zero prompt overhead) ────────────────────────

struct FuzzyMatch {
    start: usize,
    end: usize,
    match_type: String,
}

/// Three-level fuzzy fallback: whitespace → blank lines → Levenshtein similarity.
fn fuzzy_find(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    if let Some(m) = fuzzy_find_whitespace(content, old_string) {
        return Some(m);
    }
    if let Some(m) = fuzzy_find_blank_lines(content, old_string) {
        return Some(m);
    }
    let threshold = std::env::var("SKILLLITE_FUZZY_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(FUZZY_THRESHOLD);
    fuzzy_find_similarity(content, old_string, threshold)
}

/// Level 2: Ignore leading/trailing whitespace per line.
fn fuzzy_find_whitespace(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() {
        return None;
    }

    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.len() < old_lines.len() {
        return None;
    }

    let trimmed_old: Vec<&str> = old_lines.iter().map(|l| l.trim()).collect();
    if trimmed_old.iter().all(|l| l.is_empty()) {
        return None;
    }
    let offsets = line_byte_offsets(content);

    for i in 0..=(content_lines.len() - old_lines.len()) {
        let all_match = (0..old_lines.len()).all(|j| content_lines[i + j].trim() == trimmed_old[j]);

        if all_match {
            let start = offsets[i];
            let end = fuzzy_match_end(
                content,
                &offsets,
                &content_lines,
                i,
                old_lines.len(),
                old_string.ends_with('\n'),
            );
            return Some(FuzzyMatch {
                start,
                end,
                match_type: "whitespace_fuzzy".to_string(),
            });
        }
    }
    None
}

/// Level 3: Ignore blank line differences between old_string and content.
fn fuzzy_find_blank_lines(content: &str, old_string: &str) -> Option<FuzzyMatch> {
    let old_non_blank: Vec<&str> = old_string
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if old_non_blank.is_empty() {
        return None;
    }

    let content_lines: Vec<&str> = content.lines().collect();
    let offsets = line_byte_offsets(content);

    for start_line in 0..content_lines.len() {
        if content_lines[start_line].trim().is_empty() {
            continue;
        }

        let mut old_idx = 0;
        let mut last_matched_line = start_line;

        for i in start_line..content_lines.len() {
            if content_lines[i].trim().is_empty() {
                continue;
            }
            if old_idx < old_non_blank.len() && content_lines[i] == old_non_blank[old_idx] {
                old_idx += 1;
                last_matched_line = i;
            } else {
                break;
            }
        }

        if old_idx == old_non_blank.len() {
            let start = offsets[start_line];
            let end = fuzzy_match_end(
                content,
                &offsets,
                &content_lines,
                last_matched_line,
                1,
                old_string.ends_with('\n'),
            );
            return Some(FuzzyMatch {
                start,
                end,
                match_type: "blank_line_fuzzy".to_string(),
            });
        }
    }
    None
}

/// Level 4: Sliding-window Levenshtein similarity (per-line average ≥ threshold).
fn fuzzy_find_similarity(
    content: &str,
    old_string: &str,
    threshold: f64,
) -> Option<FuzzyMatch> {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() {
        return None;
    }

    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.len() < old_lines.len() {
        return None;
    }

    let offsets = line_byte_offsets(content);
    let mut best_score = 0.0_f64;
    let mut best_pos = 0_usize;

    for i in 0..=(content_lines.len() - old_lines.len()) {
        let mut total_sim = 0.0;
        for j in 0..old_lines.len() {
            total_sim += levenshtein_similarity(
                old_lines[j].trim(),
                content_lines[i + j].trim(),
            );
        }
        let avg_sim = total_sim / old_lines.len() as f64;
        if avg_sim > best_score {
            best_score = avg_sim;
            best_pos = i;
        }
    }

    if best_score >= threshold {
        let start = offsets[best_pos];
        let end = fuzzy_match_end(
            content,
            &offsets,
            &content_lines,
            best_pos,
            old_lines.len(),
            old_string.ends_with('\n'),
        );
        Some(FuzzyMatch {
            start,
            end,
            match_type: format!("similarity({:.2})", best_score),
        })
    } else {
        None
    }
}

fn fuzzy_match_end(
    content: &str,
    offsets: &[usize],
    content_lines: &[&str],
    start_line: usize,
    num_lines: usize,
    old_ends_with_newline: bool,
) -> usize {
    let end_line_idx = start_line + num_lines;
    if old_ends_with_newline {
        offsets
            .get(end_line_idx)
            .copied()
            .unwrap_or(content.len())
    } else {
        let last = start_line + num_lines - 1;
        (offsets[last] + content_lines[last].len()).min(content.len())
    }
}

fn line_byte_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn levenshtein_similarity(a: &str, b: &str) -> f64 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein_distance(a, b);
    1.0 - dist as f64 / max_len as f64
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for i in 0..a_len {
        curr[0] = i + 1;
        for j in 0..b_len {
            let cost = if a_chars[i] == b_chars[j] { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(curr[j] + 1).min(prev[j + 1] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

// ─── Helpers ────────────────────────────────────────────────────────────────

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
