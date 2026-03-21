//! search_replace / preview_edit / insert_lines: 薄封装，调用 skilllite_fs

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use super::super::{
    get_path_arg, is_key_write_path, is_sensitive_write_path, resolve_within_workspace_or_output,
};
use crate::high_risk;
use crate::types::EventSink;

pub(super) fn execute_search_replace(
    args: &Value,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
    execute_replace_like(args, workspace, true, event_sink)
}

pub(super) fn execute_preview_edit(args: &Value, workspace: &Path) -> Result<String> {
    execute_replace_like(args, workspace, false, None)
}

pub(super) fn execute_insert_lines(
    args: &Value,
    workspace: &Path,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
    let path_str =
        get_path_arg(args, false).ok_or_else(|| anyhow::anyhow!("'path' is required"))?;
    let line_num = args
        .get("line")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("'line' is required (0 = beginning of file)"))?
        as usize;
    let insert_content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;

    let workspace_str = workspace.to_string_lossy();
    if is_sensitive_write_path(&path_str) {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            "insert_lines",
            "sensitive_path_blocked",
            Some(workspace_str.as_ref()),
        );
        anyhow::bail!(
            "Blocked: editing sensitive file '{}' is not allowed",
            path_str
        );
    }
    if high_risk::confirm_write_key_path() && is_key_write_path(&path_str) {
        if let Some(sink) = event_sink {
            let msg = format!(
                "⚠️ 关键路径编辑确认\n\n路径: {}\n操作: insert_lines (在第 {} 行后插入)\n\n确认执行?",
                path_str,
                args.get("line").and_then(|v| v.as_u64()).unwrap_or(0)
            );
            if !sink.on_confirmation_request(&msg) {
                return Ok("User cancelled: edit to key path not confirmed".to_string());
            }
        }
    }

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;
    if !resolved.exists() {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            "insert_lines",
            "file_not_found",
            Some(workspace_str.as_ref()),
        );
        anyhow::bail!("File not found: {}", path_str);
    }

    let content = skilllite_fs::read_file(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;
    let new_content = skilllite_fs::insert_lines_at(&content, line_num, insert_content)?;
    let inserted_lines = insert_content.lines().count().max(1);

    let backup = backup_file_before_edit(&resolved);
    skilllite_fs::write_file(&resolved, &new_content)
        .with_context(|| format!("Failed to write file: {}", path_str))?;

    let insert_preview: String = insert_content.chars().take(200).collect();
    let diff_excerpt = if insert_content.len() > 200 {
        format!("+ {}...", insert_preview)
    } else {
        format!("+ {}", insert_preview)
    };
    skilllite_core::observability::audit_edit_inserted(
        &path_str,
        line_num,
        inserted_lines,
        &diff_excerpt,
        Some(workspace_str.as_ref()),
    );

    let validation_warning = validate_syntax(&resolved, &new_content);
    let lines = content.lines().count();
    let result = json!({
        "path": path_str,
        "inserted_after_line": line_num,
        "lines_inserted": inserted_lines,
        "new_total_lines": lines + inserted_lines,
        "backup": backup,
        "validation_warning": validation_warning
    });
    Ok(format!(
        "Successfully inserted {} line(s) after line {} in {}\n{}",
        inserted_lines,
        line_num,
        path_str,
        serde_json::to_string_pretty(&result)?
    ))
}

fn execute_replace_like(
    args: &Value,
    workspace: &Path,
    apply_changes: bool,
    event_sink: Option<&mut dyn EventSink>,
) -> Result<String> {
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
    let normalize_whitespace = args
        .get("normalize_whitespace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let workspace_str = workspace.to_string_lossy().to_string();
    if is_sensitive_write_path(&path_str) {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            "sensitive_path_blocked",
            Some(&workspace_str),
        );
        anyhow::bail!(
            "Blocked: editing sensitive file '{}' is not allowed",
            path_str
        );
    }
    if should_write && high_risk::confirm_write_key_path() && is_key_write_path(&path_str) {
        if let Some(sink) = event_sink {
            let preview = new_string.chars().take(100).collect::<String>();
            let suffix = if new_string.len() > 100 { "..." } else { "" };
            let msg = format!(
                "⚠️ 关键路径编辑确认\n\n路径: {}\n操作: search_replace\n替换预览: {}{}\n\n确认执行?",
                path_str, preview, suffix
            );
            if !sink.on_confirmation_request(&msg) {
                return Ok("User cancelled: edit to key path not confirmed".to_string());
            }
        }
    }

    let resolved = resolve_within_workspace_or_output(&path_str, workspace)?;
    if !resolved.exists() {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            "file_not_found",
            Some(&workspace_str),
        );
        anyhow::bail!("File not found: {}", path_str);
    }
    if resolved.is_dir() {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            "path_is_directory",
            Some(&workspace_str),
        );
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    let content = skilllite_fs::read_file(&resolved)
        .with_context(|| format!("Failed to read file: {}", path_str))?;

    let result = if normalize_whitespace {
        skilllite_fs::apply_replace_normalize_whitespace(
            &content,
            old_string,
            new_string,
            replace_all,
        )
    } else {
        skilllite_fs::apply_replace_fuzzy(&content, old_string, new_string, replace_all)
    }
    .inspect_err(|e| {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            &e.to_string(),
            Some(&workspace_str),
        );
    })?;

    if content == result.new_content {
        skilllite_core::observability::audit_edit_failed(
            &path_str,
            tool_name,
            "no_change_produced",
            Some(&workspace_str),
        );
        anyhow::bail!("No changes were made: replacement produced identical content");
    }

    let first_changed_line = content[..result.first_match_start]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1;
    let old_excerpt = skilllite_fs::safe_excerpt(
        &content,
        result.first_match_start,
        result.first_match_len,
        200,
    );
    let new_excerpt = skilllite_fs::safe_excerpt(
        &result.new_content,
        result.first_match_start,
        new_string.len(),
        200,
    );
    let diff_excerpt = format!("- {}\n+ {}", old_excerpt, new_excerpt);

    let mut backup: Option<String> = None;
    let mut validation_warning: Option<String> = None;

    if should_write {
        backup = backup_file_before_edit(&resolved);
        skilllite_fs::write_file(&resolved, &result.new_content)
            .with_context(|| format!("Failed to write file: {}", path_str))?;
        validation_warning = validate_syntax(&resolved, &result.new_content);
        skilllite_core::observability::audit_edit_applied(
            &path_str,
            result.replaced_count,
            first_changed_line,
            &diff_excerpt,
            Some(&workspace_str),
        );
    } else {
        skilllite_core::observability::audit_edit_previewed(
            &path_str,
            result.replaced_count,
            first_changed_line,
            &diff_excerpt,
            Some(&workspace_str),
        );
    }

    let json_result = json!({
        "path": path_str,
        "changed": true,
        "match_type": result.match_type,
        "occurrences": result.replaced_count,
        "total_occurrences": result.total_occurrences,
        "first_changed_line": first_changed_line,
        "diff_excerpt": diff_excerpt,
        "backup": backup,
        "validation_warning": validation_warning
    });

    if should_write {
        Ok(format!(
            "Successfully replaced {} occurrence(s) in {}\n{}",
            result.replaced_count,
            path_str,
            serde_json::to_string_pretty(&json_result)?
        ))
    } else {
        Ok(format!(
            "Preview edit for {} (no changes written)\n{}",
            path_str,
            serde_json::to_string_pretty(&json_result)?
        ))
    }
}

fn backup_file_before_edit(resolved: &Path) -> Option<String> {
    let backup_dir = skilllite_executor::skilllite_data_root().join("edit-backups");
    let path = skilllite_fs::backup_file(resolved, &backup_dir).ok()?;
    skilllite_fs::prune_oldest_files(&backup_dir, 50);
    Some(path.to_string_lossy().to_string())
}

fn validate_syntax(path: &Path, content: &str) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    if let Some(ref ext) = ext {
        match ext.as_str() {
            "json" => {
                if let Err(e) = serde_json::from_str::<Value>(content) {
                    return Some(format!("JSON syntax warning: {}", e));
                }
            }
            "yaml" | "yml" => {
                if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(content) {
                    return Some(format!("YAML syntax warning: {}", e));
                }
            }
            _ => {}
        }
    }
    check_bracket_balance(content)
}

fn check_bracket_balance(content: &str) -> Option<String> {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut string_char = '"';
    let mut escaped = false;
    for (line_idx, line) in content.lines().enumerate() {
        for ch in line.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && in_string {
                escaped = true;
                continue;
            }
            if in_string {
                if ch == string_char {
                    in_string = false;
                }
                continue;
            }
            match ch {
                '"' | '\'' => {
                    in_string = true;
                    string_char = ch;
                }
                '(' | '[' | '{' => stack.push((ch, line_idx + 1)),
                ')' | ']' | '}' => {
                    let expected = match ch {
                        ')' => '(',
                        ']' => '[',
                        '}' => '{',
                        _ => unreachable!(),
                    };
                    match stack.pop() {
                        Some((open, _)) if open == expected => {}
                        Some((open, open_line)) => {
                            return Some(format!(
                                "Bracket mismatch: '{}' at line {} does not match '{}' at line {}",
                                ch,
                                line_idx + 1,
                                open,
                                open_line
                            ));
                        }
                        None => {
                            return Some(format!(
                                "Unmatched closing '{}' at line {}",
                                ch,
                                line_idx + 1
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    stack
        .last()
        .map(|(open, line)| format!("Unclosed '{}' at line {}", open, line))
}
