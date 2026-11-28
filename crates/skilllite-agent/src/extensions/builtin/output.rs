//! Output tools: write_output (deliverable files), list_output.

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::types::{self, FunctionDef, ToolDefinition};

use super::{normalize_path, reject_symlink_escape};

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
    let append = args
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let explicit_output_dir = types::get_output_dir();
    let output_root = match &explicit_output_dir {
        Some(dir) => PathBuf::from(dir),
        None => workspace.join("output"),
    };
    if explicit_output_dir.is_none() {
        reject_default_output_root_escape(workspace, &output_root)?;
    }

    let input = Path::new(file_path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        output_root.join(input)
    };

    let normalized = normalize_path(&resolved);
    if !normalized.starts_with(&output_root) {
        bail!(
            "Path escapes output directory: {} (output_root: {})",
            file_path,
            output_root.display()
        );
    }
    reject_symlink_escape(&output_root, &normalized)?;

    if append {
        skilllite_fs::append_file(&normalized, content).with_context(|| {
            format!("Failed to append to output file: {}", normalized.display())
        })?;
    } else {
        skilllite_fs::write_file(&normalized, content)
            .with_context(|| format!("Failed to write output file: {}", normalized.display()))?;
    }

    Ok(format!(
        "Successfully {} {} bytes to {}",
        if append { "appended" } else { "wrote" },
        content.len(),
        normalized.display()
    ))
}

/// The default `<workspace>/output` directory must stay inside the workspace.
/// An explicit `SKILLLITE_OUTPUT_DIR` may point elsewhere on purpose.
fn reject_default_output_root_escape(workspace: &Path, output_root: &Path) -> Result<()> {
    if !output_root.exists() {
        return Ok(());
    }
    let root_canon = output_root.canonicalize().map_err(|err| {
        crate::Error::validation(format!(
            "Failed to resolve output directory {}: {err}",
            output_root.display()
        ))
    })?;
    let workspace_canon = workspace.canonicalize().map_err(|err| {
        crate::Error::validation(format!(
            "Failed to resolve workspace {}: {err}",
            workspace.display()
        ))
    })?;
    if root_canon.starts_with(&workspace_canon) {
        return Ok(());
    }
    bail!(
        "Output directory escapes workspace via symlink: {} -> {}",
        output_root.display(),
        root_canon.display()
    )
}

pub(super) fn execute_list_output(args: &Value) -> Result<String> {
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let output_root = types::get_output_dir().map(PathBuf::from).ok_or_else(|| {
        crate::Error::validation("Output directory not configured (SKILLLITE_OUTPUT_DIR)")
    })?;

    let entries = match skilllite_fs::list_directory(&output_root, recursive) {
        Ok(e) => e,
        Err(_) => return Ok("Output directory does not exist or is empty.".to_string()),
    };

    if entries.is_empty() {
        return Ok("Output directory is empty.".to_string());
    }

    Ok(entries.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::execute_write_output;
    use serde_json::json;
    use std::fs;

    fn write_output(
        workspace: &std::path::Path,
        file_path: &str,
        content: &str,
        append: bool,
    ) -> crate::Result<String> {
        execute_write_output(
            &json!({
                "file_path": file_path,
                "content": content,
                "append": append
            }),
            workspace,
        )
    }

    #[test]
    fn write_output_keeps_unicode_file_inside_the_output_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let written = write_output(workspace.path(), "报告.md", "中文内容", false).expect("write");
        assert!(written.contains("报告.md"), "written={written}");
        assert_eq!(
            fs::read_to_string(workspace.path().join("output").join("报告.md")).expect("read"),
            "中文内容"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_output_rejects_symlink_that_points_outside_the_output_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let output = workspace.path().join("output");
        fs::create_dir_all(&output).expect("output dir");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), "保留 中文").expect("seed outside");
        std::os::unix::fs::symlink(outside.path(), output.join("leak")).expect("symlink");

        let err = write_output(workspace.path(), "leak", "pwned", false)
            .expect_err("symlink write must fail");
        assert!(err.to_string().contains("symlink"), "err={err}");
        assert_eq!(
            fs::read_to_string(outside.path()).expect("read outside"),
            "保留 中文"
        );

        let err = write_output(workspace.path(), "leak", "more", true)
            .expect_err("symlink append must fail");
        assert!(err.to_string().contains("symlink"), "err={err}");
        assert_eq!(
            fs::read_to_string(outside.path()).expect("read outside"),
            "保留 中文"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_output_rejects_a_new_file_under_a_symlinked_directory() {
        let workspace = tempfile::tempdir().expect("workspace");
        let output = workspace.path().join("output");
        fs::create_dir_all(&output).expect("output dir");
        let outside = tempfile::tempdir().expect("outside dir");
        std::os::unix::fs::symlink(outside.path(), output.join("linked")).expect("symlink dir");

        let err = write_output(workspace.path(), "linked/pwn.txt", "pwned", false)
            .expect_err("symlink dir write must fail");
        assert!(err.to_string().contains("symlink"), "err={err}");
        assert!(!outside.path().join("pwn.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_output_rejects_when_the_default_output_directory_is_an_outside_symlink() {
        let workspace = tempfile::tempdir().expect("workspace");
        let outside = tempfile::tempdir().expect("outside dir");
        std::os::unix::fs::symlink(outside.path(), workspace.path().join("output"))
            .expect("symlink output");

        let err = write_output(workspace.path(), "note.txt", "pwned", false)
            .expect_err("output root symlink must fail");
        assert!(err.to_string().contains("symlink"), "err={err}");
        assert!(!outside.path().join("note.txt").exists());
    }
}
