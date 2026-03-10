//! grep_files: 薄封装，调用 skilllite_fs::grep_directory

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

use super::super::resolve_within_workspace_or_output;

pub(super) fn execute_grep_files(args: &Value, workspace: &Path) -> Result<String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .context("'pattern' is required")?;
    let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let include = args.get("include").and_then(|v| v.as_str());

    let re = regex::Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {}", e))?;

    let resolved = resolve_within_workspace_or_output(path_str, workspace)?;
    if !resolved.exists() {
        anyhow::bail!("Path not found: {}", path_str);
    }

    const MAX_MATCHES: usize = 50;
    let (results, files_matched) = skilllite_fs::grep_directory(
        &resolved,
        &re,
        Some(workspace),
        include,
        skilllite_fs::SKIP_DIRS,
        MAX_MATCHES,
    )?;

    if results.is_empty() {
        return Ok("No matches found.".to_string());
    }

    let total = results.len();
    let output: Vec<String> = results
        .into_iter()
        .map(|(rel, line_num, line)| format!("{}:{}:{}", rel, line_num, line))
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
    Ok(out)
}
