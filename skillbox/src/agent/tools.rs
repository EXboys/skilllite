//! Built-in tools for the agent.
//!
//! Phase 1: read_file, write_file, list_directory, file_exists
//! Phase 2: run_command, write_output, preview_server (stub)
//!
//! Ported from Python `builtin_tools.py`. Enforces workspace confinement
//! and sensitive path blocking.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::types::{self, EventSink, ToolDefinition, FunctionDef, ToolResult};

// ─── Security helpers (ported from Python) ──────────────────────────────────

/// Sensitive file patterns that should never be written to.
const SENSITIVE_PATTERNS: &[&str] = &[".env", ".git/config", ".key"];

/// Check if a path is sensitive and should be blocked for writes.
fn is_sensitive_write_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.ends_with(pattern) || lower.contains(&format!("{}/", pattern)) {
            return true;
        }
    }
    // Also block *.key files
    if lower.ends_with(".key") || lower.ends_with(".pem") {
        return true;
    }
    false
}

/// Resolve a path and ensure it stays within the workspace root.
/// Prevents path traversal attacks (e.g. "../../etc/passwd").
fn resolve_within_workspace(path: &str, workspace: &Path) -> Result<PathBuf> {
    let input = Path::new(path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        workspace.join(input)
    };

    // Normalize by resolving ".." components without requiring the path to exist
    let normalized = normalize_path(&resolved);

    if !normalized.starts_with(workspace) {
        anyhow::bail!(
            "Path escapes workspace: {} (workspace: {})",
            path,
            workspace.display()
        );
    }

    Ok(normalized)
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

// ─── Tool definitions ───────────────────────────────────────────────────────

/// Get all built-in tool definitions in OpenAI function-calling format.
pub fn get_builtin_tool_definitions() -> Vec<ToolDefinition> {
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
                description: "Write content to a file. Creates parent directories if needed. Blocks writes to sensitive files (.env, .key, .git/config).".to_string(),
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
                        }
                    },
                    "required": ["path", "content"]
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
        // ── Phase 2 tools ──────────────────────────────────────────────
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "run_command".to_string(),
                description: "Execute a shell command in the workspace directory. Requires user confirmation before execution. Dangerous commands (rm -rf, curl|bash, etc.) are flagged with extra warnings. Timeout: 300 seconds.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute"
                        }
                    },
                    "required": ["command"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "write_output".to_string(),
                description: "Write final output to the output directory. Use for deliverable files (HTML, reports, etc.). Path is relative to the output directory.".to_string(),
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
                        }
                    },
                    "required": ["file_path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "preview_server".to_string(),
                description: "Start a local HTTP server to preview HTML files in the browser. Specify the directory to serve.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Directory to serve (relative to workspace)"
                        },
                        "port": {
                            "type": "integer",
                            "description": "Port number (default: 8765)"
                        }
                    },
                    "required": ["directory_path"]
                }),
            },
        },
    ]
}

// ─── Tool execution ─────────────────────────────────────────────────────────

/// Check if a tool name is a built-in tool.
pub fn is_builtin_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "write_file"
            | "list_directory"
            | "file_exists"
            | "run_command"
            | "write_output"
            | "preview_server"
    )
}

/// Check if a built-in tool requires async execution (uses EventSink).
pub fn is_async_builtin_tool(name: &str) -> bool {
    matches!(name, "run_command" | "preview_server")
}

/// Execute a synchronous built-in tool. Returns the result content string.
/// For async tools (run_command, preview_server), use `execute_async_builtin_tool`.
pub fn execute_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
) -> ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Invalid arguments JSON: {}", e),
                is_error: true,
            };
        }
    };

    let result = match tool_name {
        "read_file" => execute_read_file(&args, workspace),
        "write_file" => execute_write_file(&args, workspace),
        "list_directory" => execute_list_directory(&args, workspace),
        "file_exists" => execute_file_exists(&args, workspace),
        "write_output" => execute_write_output(&args, workspace),
        _ => Err(anyhow::anyhow!("Unknown built-in tool: {}", tool_name)),
    };

    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
        },
    }
}

/// Execute an async built-in tool (run_command, preview_server).
/// These tools require `EventSink` for user confirmation or streaming output.
pub async fn execute_async_builtin_tool(
    tool_name: &str,
    arguments: &str,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> ToolResult {
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Invalid arguments JSON: {}", e),
                is_error: true,
            };
        }
    };

    let result = match tool_name {
        "run_command" => execute_run_command(&args, workspace, event_sink).await,
        "preview_server" => execute_preview_server(&args, workspace),
        _ => Err(anyhow::anyhow!("Unknown async built-in tool: {}", tool_name)),
    };

    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
        },
    }
}

/// Read a file's UTF-8 content.
fn execute_read_file(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .context("'path' is required")?;

    let resolved = resolve_within_workspace(path_str, workspace)?;

    if !resolved.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }

    if resolved.is_dir() {
        anyhow::bail!("Path is a directory, not a file: {}", path_str);
    }

    // Try reading as UTF-8, fall back to noting it's binary
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

/// Write content to a file. Blocks sensitive paths.
fn execute_write_file(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .context("'path' is required")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;

    if is_sensitive_write_path(path_str) {
        anyhow::bail!(
            "Blocked: writing to sensitive file '{}' is not allowed",
            path_str
        );
    }

    let resolved = resolve_within_workspace(path_str, workspace)?;

    // Create parent directories
    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(&resolved, content)
        .with_context(|| format!("Failed to write file: {}", resolved.display()))?;

    Ok(format!(
        "Successfully wrote {} bytes to {}",
        content.len(),
        path_str
    ))
}

/// List directory contents.
fn execute_list_directory(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let resolved = resolve_within_workspace(path_str, workspace)?;

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

fn list_dir_impl(
    base: &Path,
    current: &Path,
    recursive: bool,
    entries: &mut Vec<String>,
    depth: usize,
) -> Result<()> {
    let mut items: Vec<_> = std::fs::read_dir(current)?
        .filter_map(|e| e.ok())
        .collect();
    items.sort_by_key(|e| e.file_name());

    // Skip hidden and common non-essential directories
    let skip_dirs = [
        "node_modules",
        "__pycache__",
        ".git",
        "venv",
        ".venv",
        ".tox",
        "target",
    ];

    for entry in items {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && depth == 0 && name != "." {
            // Show top-level hidden dirs but don't recurse
            let prefix = if entry.path().is_dir() { "📁 " } else { "   " };
            entries.push(format!("{}{}", prefix, name));
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(base)
            .unwrap_or(&entry.path())
            .to_string_lossy()
            .to_string();

        if entry.path().is_dir() {
            entries.push(format!("📁 {}/", rel));
            if recursive && !skip_dirs.contains(&name.as_str()) {
                list_dir_impl(base, &entry.path(), true, entries, depth + 1)?;
            }
        } else {
            let meta = entry.metadata().ok();
            let size = meta.map(|m| m.len()).unwrap_or(0);
            entries.push(format!("   {} ({})", rel, format_size(size)));
        }
    }

    Ok(())
}

/// Check if a file or directory exists.
fn execute_file_exists(args: &Value, workspace: &Path) -> Result<String> {
    let path_str = args
        .get("path")
        .and_then(|v| v.as_str())
        .context("'path' is required")?;

    let resolved = resolve_within_workspace(path_str, workspace)?;

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

/// Format byte size into human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ─── Phase 2: run_command ────────────────────────────────────────────────────

/// Dangerous command regex patterns.
/// Ported from Python `_check_dangerous_command`.
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*--force)", "rm with force flag — may delete files irreversibly"),
    (r"rm\s+-[a-zA-Z]*r[a-zA-Z]*\s+/\s*$", "rm -rf / — system destruction"),
    (r"(curl|wget)\s+.*\|\s*(bash|sh|zsh)", "piping remote script to shell — remote code execution risk"),
    (r":\(\)\s*\{\s*:\|:\s*&\s*\}\s*;\s*:", "fork bomb — will crash the system"),
    (r"chmod\s+(-[a-zA-Z]*R|--recursive)\s+777", "recursive chmod 777 — insecure permission change"),
];

/// Check if a command is dangerous. Returns a warning reason if so.
fn check_dangerous_command(cmd: &str) -> Option<String> {
    for (pattern, reason) in DANGEROUS_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(cmd) {
                return Some(reason.to_string());
            }
        }
    }
    None
}

/// Execute `run_command`: shell command with confirmation + timeout.
/// Ported from Python `builtin_tools.py` run_command implementation.
async fn execute_run_command(
    args: &Value,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> Result<String> {
    let cmd = args
        .get("command")
        .and_then(|v| v.as_str())
        .context("'command' is required")?;

    if cmd.trim().is_empty() {
        anyhow::bail!("command must not be empty");
    }

    // Build confirmation message
    let confirm_msg = if let Some(danger_reason) = check_dangerous_command(cmd) {
        format!(
            "⚠️ Dangerous command detected\n\n\
             Pattern that may cause serious harm: {}\n\n\
             Command: {}\n\n\
             Please verify before confirming execution.",
            danger_reason, cmd
        )
    } else {
        format!("About to execute command:\n  {}\n\nConfirm execution?", cmd)
    };

    // Request user confirmation
    if !event_sink.on_confirmation_request(&confirm_msg) {
        return Ok("User cancelled command execution".to_string());
    }

    // Execute command via tokio subprocess
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(workspace)
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;

    // Read stdout + stderr concurrently, stream to event_sink
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut output_lines = Vec::new();

    // Read stdout
    if let Some(stdout) = stdout {
        let mut reader = BufReader::new(stdout).lines();
        // Note: we can't stream to event_sink here because it requires &mut.
        // Collect all output, then report.
        while let Ok(Some(line)) = reader.next_line().await {
            output_lines.push(line);
        }
    }

    // Read stderr
    let mut stderr_lines = Vec::new();
    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            stderr_lines.push(line);
        }
    }

    // Wait for process with timeout (300 seconds)
    let timeout_duration = tokio::time::Duration::from_secs(300);
    let status = match tokio::time::timeout(timeout_duration, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => {
            return Ok(format!("Error waiting for command: {}", e));
        }
        Err(_) => {
            // Timeout — kill the process
            let _ = child.kill().await;
            return Ok("Error: Command execution timeout (300s)".to_string());
        }
    };

    // Build result
    let stdout_text = output_lines.join("\n");
    let stderr_text = stderr_lines.join("\n");
    let mut result = String::new();

    if status.success() {
        if stdout_text.is_empty() && stderr_text.is_empty() {
            result.push_str("Command succeeded (exit 0)");
        } else {
            result.push_str(&format!("Command succeeded (exit 0):\n{}", stdout_text));
            if !stderr_text.is_empty() {
                result.push_str(&format!("\n[stderr]: {}", stderr_text));
            }
        }
    } else {
        let code = status.code().unwrap_or(-1);
        let combined = if !stdout_text.is_empty() && !stderr_text.is_empty() {
            format!("{}\n[stderr]: {}", stdout_text, stderr_text)
        } else if !stderr_text.is_empty() {
            stderr_text
        } else {
            stdout_text
        };
        result.push_str(&format!("Command failed (exit {}):\n{}", code, combined));
    }

    Ok(result)
}

// ─── Phase 2: write_output ──────────────────────────────────────────────────

/// Execute `write_output`: write deliverable files to the output directory.
/// Ported from Python `builtin_tools.py` write_output implementation.
fn execute_write_output(args: &Value, workspace: &Path) -> Result<String> {
    let file_path = args
        .get("file_path")
        .and_then(|v| v.as_str())
        .context("'file_path' is required")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;

    // Resolve output directory: SKILLLITE_OUTPUT_DIR > {workspace}/output
    let output_root = match types::get_output_dir() {
        Some(dir) => PathBuf::from(dir),
        None => workspace.join("output"),
    };

    // Resolve path within output_root
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

    // Create parent directories
    if let Some(parent) = normalized.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(&normalized, content)
        .with_context(|| format!("Failed to write output file: {}", normalized.display()))?;

    Ok(format!(
        "Successfully wrote {} bytes to {}",
        content.len(),
        normalized.display()
    ))
}

// ─── Phase 2: preview_server (stub) ─────────────────────────────────────────

/// Execute `preview_server` — currently a stub that suggests alternative.
/// Full implementation deferred to Phase 2+ / Phase 3.
fn execute_preview_server(_args: &Value, _workspace: &Path) -> Result<String> {
    Ok(
        "preview_server is not yet available in the Rust agent. \
         As an alternative, you can use `run_command` with:\n  \
         python -m http.server 8765 --directory <dir>\n\
         or:\n  \
         npx serve <dir> -l 8765"
            .to_string(),
    )
}

// ─── Long content handling ──────────────────────────────────────────────────

/// Process tool result content: truncate if too long.
///
/// This is the **synchronous** fast path. Returns `Some(truncated)` if the
/// content was handled (either unchanged or truncated). Returns `None` if the
/// content exceeds the summarization threshold and should be handled by the
/// async `long_text::summarize_long_content` in `agent_loop`.
///
/// Ported from Python `_process_tool_result_content` with the addition of
/// env-configurable thresholds (Phase 2).
pub fn process_tool_result_content(content: &str) -> Option<String> {
    let max_chars = types::get_tool_result_max_chars();
    let summarize_threshold = types::get_summarize_threshold();
    let len = content.len();

    if len <= max_chars {
        return Some(content.to_string());
    }

    if len > summarize_threshold {
        // Signal caller to use async LLM summarization
        return None;
    }

    // Between max_chars and summarize_threshold: simple truncation
    Some(format!(
        "{}\n\n[... 结果已截断，原文共 {} 字符，仅保留前 {} 字符 ...]",
        &content[..max_chars],
        len,
        max_chars
    ))
}

/// Synchronous fallback: head+tail truncation for content that exceeds the
/// summarize threshold but where LLM summarization is not available or failed.
pub fn process_tool_result_content_fallback(content: &str) -> String {
    let max_chars = types::get_tool_result_max_chars();
    let len = content.len();

    if len <= max_chars {
        return content.to_string();
    }

    let head_size = max_chars.min(len);
    let tail_size = (max_chars / 3).min(len);
    let head = &content[..head_size];
    let tail = &content[len.saturating_sub(tail_size)..];
    format!(
        "{}\n\n... [content truncated: {} chars total, showing head+tail] ...\n\n{}",
        head, len, tail
    )
}
