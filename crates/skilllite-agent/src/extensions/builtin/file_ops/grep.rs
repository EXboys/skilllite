//! grep_files: 薄封装，调用 skilllite_fs::grep_directory

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde_json::Value;
use std::path::Path;

use super::super::{
    filter_sensitive_content_in_text, is_sensitive_read_path, resolve_within_workspace_or_output,
};

pub(super) fn execute_grep_files(args: &Value, workspace: &Path) -> Result<String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .context("'pattern' is required")?;
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let include = args.get("include").and_then(|v| v.as_str());

    let re = regex::Regex::new(pattern)
        .map_err(|e| crate::Error::validation(format!("Invalid regex pattern: {}", e)))?;

    let resolved = resolve_within_workspace_or_output(path_str, workspace)?;
    if !resolved.exists() {
        bail!("Path not found: {}", path_str);
    }

    // Align with read_file: refuse a direct sensitive target.
    if is_sensitive_read_path(path_str) {
        bail!(
            "Blocked: reading sensitive file '{}' (.env, .key, .git/config, etc.) is not allowed",
            path_str
        );
    }

    const MAX_MATCHES: usize = 50;
    let skip_sensitive = |path: &Path| {
        let rel = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy();
        is_sensitive_read_path(&rel)
    };
    let (results, files_matched) = skilllite_fs::grep_directory(
        &resolved,
        &re,
        Some(workspace),
        include,
        skilllite_fs::SKIP_DIRS,
        MAX_MATCHES,
        Some(&skip_sensitive),
    )?;

    if results.is_empty() {
        return Ok("No matches found.".to_string());
    }

    let total = results.len();
    let mut redacted_any = false;
    let output: Vec<String> = results
        .into_iter()
        .map(|(rel, line_num, line)| {
            let (filtered, was_redacted) = filter_sensitive_content_in_text(&line);
            if was_redacted {
                redacted_any = true;
            }
            format!("{}:{}:{}", rel, line_num, filtered)
        })
        .collect();
    let mut out = output.join("\n");
    out.push_str(&format!(
        "\n\n[{} match(es) in {} file(s){}]",
        total,
        files_matched,
        if total >= MAX_MATCHES {
            " — results capped at 50"
        } else {
            ""
        }
    ));
    if redacted_any {
        out.push_str("\n\n[⚠️ Sensitive values (API_KEY, PASSWORD, etc.) have been redacted]");
    }
    Ok(out)
}
