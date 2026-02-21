//! Output tools: write_output (deliverable files), list_output.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::agent::types::{self, ToolDefinition, FunctionDef};

use super::{normalize_path, list_dir_impl};

// ─── Tool definitions ───────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "write_output".to_string(),
                description: "Write final output to the output directory. Use for deliverable files (HTML, reports, etc.). Path is relative to the output directory. Use append: true to append to existing file. For content >~6k chars, split into multiple calls: first call overwrites, subsequent calls use append: true.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "File path relative to the output directory"
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
                    "required": ["file_path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "list_output".to_string(),
                description: "List files in the output directory (where write_output saves files). Use when the user asks what files were generated, or to find output files by name. No path needed.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "recursive": {
                            "type": "boolean",
                            "description": "If true, list recursively. Default: false."
                        }
                    },
                    "required": []
                }),
            },
        },
    ]
}

// ─── Execution ──────────────────────────────────────────────────────────────

pub(super) fn execute_write_output(args: &Value, workspace: &Path) -> Result<String> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .context("'file_path' is required")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;
    let append = args.get("append").and_then(|v| v.as_bool()).unwrap_or(false);

    let output_root = match types::get_output_dir() {
        Some(dir) => PathBuf::from(dir),
        None => workspace.join("output"),
    };

    let input = Path::new(file_path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        output_root.join(input)
    };

    let normalized = normalize_path(&resolved);
    if !normalized.starts_with(&output_root) {
        anyhow::bail!(
            "Path escapes output directory: {} (output_root: {})",
            file_path,
            output_root.display()
        );
    }

    if let Some(parent) = normalized.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    if append {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&normalized)
            .with_context(|| format!("Failed to open output file for append: {}", normalized.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("Failed to append to output file: {}", normalized.display()))?;
    } else {
        std::fs::write(&normalized, content)
            .with_context(|| format!("Failed to write output file: {}", normalized.display()))?;
    }

    Ok(format!(
        "Successfully {} {} bytes to {}",
        if append { "appended" } else { "wrote" },
        content.len(),
        normalized.display()
    ))
}

pub(super) fn execute_list_output(args: &Value) -> Result<String> {
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let output_root = types::get_output_dir()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Output directory not configured (SKILLLITE_OUTPUT_DIR)"))?;

    if !output_root.exists() {
        return Ok("Output directory does not exist or is empty.".to_string());
    }
    if !output_root.is_dir() {
        anyhow::bail!("Output path is not a directory: {}", output_root.display());
    }

    let mut entries = Vec::new();
    list_dir_impl(&output_root, &output_root, recursive, &mut entries, 0)?;

    if entries.is_empty() {
        return Ok("Output directory is empty.".to_string());
    }

    Ok(entries.join("\n"))
}
