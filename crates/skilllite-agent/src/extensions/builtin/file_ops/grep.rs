//! grep_files implementation.

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

    let mut results = Vec::new();
    let mut files_matched = 0usize;
    const MAX_MATCHES: usize = 50;

    grep_recursive(
        &resolved,
        workspace,
        &re,
        include,
        &mut results,
        &mut files_matched,
        MAX_MATCHES,
    )?;

    if results.is_empty() {
        return Ok("No matches found.".to_string());
    }

    let total = results.len();
    let mut output = results.join("\n");
    output.push_str(&format!(
        "\n\n[{} match(es) in {} file(s){}]",
        total,
        files_matched,
        if total >= MAX_MATCHES {
            " — results capped at 50"
        } else {
            ""
        }
    ));
    Ok(output)
}

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    "venv",
    ".venv",
    ".tox",
];

fn grep_recursive(
    dir: &Path,
    workspace: &Path,
    re: &regex::Regex,
    include: Option<&str>,
    results: &mut Vec<String>,
    files_matched: &mut usize,
    max_matches: usize,
) -> Result<()> {
    if !dir.is_dir() {
        return grep_single_file(dir, workspace, re, results, files_matched, max_matches);
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if results.len() >= max_matches {
            return Ok(());
        }

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            grep_recursive(&path, workspace, re, include, results, files_matched, max_matches)?;
        } else {
            if let Some(glob) = include {
                if !matches_glob(&name, glob) {
                    continue;
                }
            }
            if is_likely_binary(&path) {
                continue;
            }
            grep_single_file(&path, workspace, re, results, files_matched, max_matches)?;
        }
    }
    Ok(())
}

fn grep_single_file(
    path: &Path,
    workspace: &Path,
    re: &regex::Regex,
    results: &mut Vec<String>,
    files_matched: &mut usize,
    max_matches: usize,
) -> Result<()> {
    if let Ok(content) = std::fs::read_to_string(path) {
        let rel_path = path
            .strip_prefix(workspace)
            .unwrap_or(path)
            .to_string_lossy();
        let mut file_has_match = false;

        for (line_num, line) in content.lines().enumerate() {
            if results.len() >= max_matches {
                break;
            }
            if re.is_match(line) {
                if !file_has_match {
                    *files_matched += 1;
                    file_has_match = true;
                }
                results.push(format!("{}:{}:{}", rel_path, line_num + 1, line));
            }
        }
    }
    Ok(())
}

fn matches_glob(name: &str, pattern: &str) -> bool {
    if let Some(ext) = pattern.strip_prefix("*.") {
        name.ends_with(&format!(".{}", ext))
    } else {
        name == pattern
    }
}

fn is_likely_binary(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = [0u8; 512];
        if let Ok(n) = f.read(&mut buf) {
            return buf[..n].contains(&0);
        }
    }
    true
}

