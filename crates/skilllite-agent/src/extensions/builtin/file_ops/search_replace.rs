//! search_replace / preview_edit + fuzzy matching, backup, validation.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::super::{get_path_arg, is_sensitive_write_path, resolve_within_workspace_or_output};

const FUZZY_THRESHOLD: f64 = 0.85;

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

    let indented_content = auto_indent(insert_content, &lines, line_num);
    let effective_content = indented_content.as_deref().unwrap_or(insert_content);

    let insert_with_newline = if effective_content.ends_with('\n') {
        effective_content.to_string()
    } else {
        format!("{}\n", effective_content)
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

    let backup = backup_file_before_edit(&resolved);

    std::fs::write(&resolved, &new_content)
        .with_context(|| format!("Failed to write file: {}", path_str))?;

    let validation_warning = validate_syntax(&resolved, &new_content);

    let inserted_lines = insert_content.lines().count().max(1);
    let result = json!({
        "path": path_str,
        "inserted_after_line": line_num,
        "lines_inserted": inserted_lines,
        "new_total_lines": total + inserted_lines,
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
                None => {
                    let hint = build_failure_hint(&content, old_string);
                    Err(anyhow::anyhow!(
                        "old_string not found in file (tried exact + fuzzy matching).\n\n{}\n\nTip: Copy the exact text from above into old_string, or use insert_lines with line number.",
                        hint
                    ))
                }
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

    let mut backup: Option<String> = None;
    let mut validation_warning: Option<String> = None;

    if should_write {
        backup = backup_file_before_edit(&resolved);
        std::fs::write(&resolved, &new_content)
            .with_context(|| format!("Failed to write file: {}", path_str))?;
        validation_warning = validate_syntax(&resolved, &new_content);
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
        "diff_excerpt": diff_excerpt,
        "backup": backup,
        "validation_warning": validation_warning
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

// ─── Indent awareness (III4) ────────────────────────────────────────────────

/// If insert_content has no indentation but the target line does, auto-indent.
/// Returns None if no adjustment needed.
fn auto_indent(content: &str, lines: &[&str], after_line: usize) -> Option<String> {
    // Prefer the NEXT line as reference (the surrounding context where content lands).
    // Fall back to the line we're inserting after.
    let ref_line = if after_line < lines.len() {
        lines[after_line]
    } else if after_line > 0 {
        lines[after_line - 1]
    } else if !lines.is_empty() {
        lines[0]
    } else {
        return None;
    };

    let indent = detect_indentation(ref_line);
    if indent.is_empty() {
        return None;
    }

    let has_indent = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .any(|l| l.starts_with(' ') || l.starts_with('\t'));
    if has_indent {
        return None;
    }

    let indented: Vec<String> = content
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                format!("{}{}", indent, l)
            }
        })
        .collect();
    Some(indented.join("\n"))
}

fn detect_indentation(line: &str) -> &str {
    let trimmed_len = line.trim_start().len();
    &line[..line.len() - trimmed_len]
}

// ─── Edit failure smart hints (III1) ────────────────────────────────────────

/// When fuzzy match fails entirely, find the most similar region and show ±10 lines.
fn build_failure_hint(content: &str, old_string: &str) -> String {
    let old_lines: Vec<&str> = old_string.lines().collect();
    if old_lines.is_empty() || content.is_empty() {
        return "File is empty or old_string is empty.".to_string();
    }

    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.is_empty() {
        return "File is empty.".to_string();
    }

    let mut best_score = 0.0_f64;
    let mut best_pos = 0_usize;
    let window = old_lines.len().min(content_lines.len());

    for i in 0..=(content_lines.len().saturating_sub(window)) {
        let mut total_sim = 0.0;
        for j in 0..window {
            total_sim += levenshtein_similarity(
                old_lines.get(j).unwrap_or(&"").trim(),
                content_lines[i + j].trim(),
            );
        }
        let avg = total_sim / window as f64;
        if avg > best_score {
            best_score = avg;
            best_pos = i;
        }
    }

    let context_radius = 5;
    let ctx_start = best_pos.saturating_sub(context_radius);
    let ctx_end = (best_pos + window + context_radius).min(content_lines.len());

    let mut hint = format!(
        "Closest match found at lines {}-{} (similarity: {:.2}, below threshold):\n",
        best_pos + 1,
        best_pos + window,
        best_score
    );
    for i in ctx_start..ctx_end {
        hint.push_str(&format!("{:>6}|{}\n", i + 1, content_lines[i]));
    }
    hint
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

// ─── Auto-backup (II2) ─────────────────────────────────────────────────────

fn backup_file_before_edit(resolved: &Path) -> Option<String> {
    let home = dirs::home_dir()?;
    let backup_dir = home.join(".skilllite").join("edit-backups");
    std::fs::create_dir_all(&backup_dir).ok()?;

    let filename = resolved.file_name()?.to_string_lossy().to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_name = format!("{}_{}", ts, filename);
    let backup_path = backup_dir.join(&backup_name);

    std::fs::copy(resolved, &backup_path).ok()?;
    cleanup_old_backups(&backup_dir, 50);
    Some(backup_path.to_string_lossy().to_string())
}

fn cleanup_old_backups(dir: &Path, keep: usize) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();

        if files.len() <= keep {
            return;
        }

        files.sort_by_key(|p| {
            p.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        for path in files.iter().take(files.len() - keep) {
            let _ = std::fs::remove_file(path);
        }
    }
}

// ─── Syntax validation (II3) ───────────────────────────────────────────────

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
    let mut stack: Vec<(char, usize)> = Vec::new();
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

    if let Some((open, line)) = stack.last() {
        return Some(format!("Unclosed '{}' at line {}", open, line));
    }

    None
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn safe_excerpt(content: &str, start: usize, span_len: usize, max_len: usize) -> String {
    let prefix = 80usize;
    let suffix = 80usize;
    let begin = floor_char_boundary(content, start.saturating_sub(prefix));
    let end = ceil_char_boundary(content, (start + span_len + suffix).min(content.len()));
    let mut excerpt = content[begin..end].replace('\n', "\\n");
    if excerpt.len() > max_len {
        let safe_len = floor_char_boundary(&excerpt, max_len);
        excerpt.truncate(safe_len);
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
