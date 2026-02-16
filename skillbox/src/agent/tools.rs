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
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
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
        Err(_e) => {
            // Truncated JSON recovery: when LLM hits max_tokens (finish_reason: "length"),
            // tool arguments may be cut off mid-string. Try to recover file_path + content
            // for write_file and write_output. Ported from Python `_parse_truncated_json_for_file_tools`.
            if tool_name == "write_file" || tool_name == "write_output" {
                match parse_truncated_json_for_file_tools(arguments) {
                    Some(recovered) if recovered.as_object().map_or(false, |o| !o.is_empty()) => {
                        tracing::warn!(
                            "Recovered truncated JSON for {} ({} fields)",
                            tool_name,
                            recovered.as_object().map_or(0, |o| o.len())
                        );
                        recovered
                    }
                    _ => {
                        return ToolResult {
                            tool_call_id: String::new(),
                            tool_name: tool_name.to_string(),
                            content: format!("Invalid arguments JSON: {}", _e),
                            is_error: true,
                        };
                    }
                }
            } else {
                return ToolResult {
                    tool_call_id: String::new(),
                    tool_name: tool_name.to_string(),
                    content: format!("Invalid arguments JSON: {}", _e),
                    is_error: true,
                };
            }
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

// ─── Truncated JSON recovery ─────────────────────────────────────────────────

/// Attempt to extract file_path/path and content from truncated JSON.
/// When LLM hits max_tokens (finish_reason: "length"), tool arguments may be cut off.
/// Ported from Python `_parse_truncated_json_for_file_tools` in `core/tools.py`.
fn parse_truncated_json_for_file_tools(arguments: &str) -> Option<Value> {
    if arguments.is_empty() {
        return None;
    }

    let mut result = serde_json::Map::new();

    // Extract "path" or "file_path": "value"
    let path_re = regex::Regex::new(r#""(?:file_)?path"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = path_re.captures(arguments) {
        let key = if arguments.contains("\"file_path\"") {
            "file_path"
        } else {
            "path"
        };
        result.insert(
            key.to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    }

    // Extract "content": try complete JSON string first
    let content_complete_re =
        regex::Regex::new(r#""content"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = content_complete_re.captures(arguments) {
        result.insert(
            "content".to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    } else {
        // Truncated: match from "content": " to end of string
        let content_trunc_re = regex::Regex::new(r#""content"\s*:\s*"(.*)$"#).ok()?;
        if let Some(caps) = content_trunc_re.captures(arguments) {
            let mut raw = caps.get(1)?.as_str().to_string();
            // Strip trailing "}" or " that may be from truncated JSON structure
            if raw.ends_with("\"}") {
                raw = raw[..raw.len() - 2].to_string();
            } else if raw.ends_with('"') && !raw.ends_with("\\\"") {
                raw = raw[..raw.len() - 1].to_string();
            }
            result.insert(
                "content".to_string(),
                Value::String(unescape_json_string(&raw)),
            );
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(Value::Object(result))
    }
}

/// Unescape a JSON string value (handles \n, \", \\, \t, \r).
fn unescape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
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

// ─── Phase 2.5: preview_server ──────────────────────────────────────────────

use std::sync::Mutex;

/// Global state for active preview server (reuse / shutdown).
static ACTIVE_PREVIEW: Mutex<Option<PreviewServerState>> = Mutex::new(None);

struct PreviewServerState {
    serve_dir: String,
    port: u16,
}

/// Execute `preview_server`: start a local HTTP file server in a daemon thread.
/// Ported from Python `_start_preview_server` in builtin_tools.py.
///
/// Features:
/// - Binds to 127.0.0.1 only
/// - Auto-scans ports (default 8765, tries +19)
/// - Reuses server if same directory
/// - Shuts down old server if different directory
/// - Sends no-cache headers
/// - Opens browser via `open` / `xdg-open`
fn execute_preview_server(args: &Value, workspace: &Path) -> Result<String> {
    let dir_path = args
        .get("directory_path")
        .and_then(|v| v.as_str())
        .context("'directory_path' is required")?;
    let requested_port = args
        .get("port")
        .and_then(|v| v.as_u64())
        .unwrap_or(8765) as u16;

    let resolved = resolve_within_workspace(dir_path, workspace)?;

    // If a file path was given, serve its parent directory
    let (serve_dir, target_file) = if resolved.is_file() {
        let fname = resolved.file_name().map(|f| f.to_string_lossy().to_string());
        (resolved.parent().unwrap_or(&resolved).to_path_buf(), fname)
    } else {
        (resolved.clone(), None)
    };

    if !serve_dir.exists() {
        anyhow::bail!("Path not found: {}", dir_path);
    }

    let serve_dir_str = serve_dir.to_string_lossy().to_string();

    // Check if server already running for this directory
    {
        let guard = ACTIVE_PREVIEW.lock().unwrap();
        if let Some(ref state) = *guard {
            if state.serve_dir == serve_dir_str {
                let url = build_preview_url(state.port, target_file.as_deref());
                open_browser(&url);
                return Ok(format!(
                    "Preview server already running at {}\n\n\
                     Open in browser: {}\n\
                     Serving directory: {}\n\
                     (Browser opened with fresh page.)",
                    url, url, serve_dir_str
                ));
            }
        }
    }

    // Try to bind to a port
    let listener = {
        let mut bound = None;
        for p in requested_port..requested_port.saturating_add(20).min(65535) {
            match std::net::TcpListener::bind(("127.0.0.1", p)) {
                Ok(l) => {
                    bound = Some((l, p));
                    break;
                }
                Err(_) => continue,
            }
        }
        bound
    };

    let (listener, used_port) = match listener {
        Some((l, p)) => (l, p),
        None => anyhow::bail!(
            "Could not bind to port {} (tried {}-{})",
            requested_port,
            requested_port,
            requested_port + 19
        ),
    };

    // Update global state
    {
        let mut guard = ACTIVE_PREVIEW.lock().unwrap();
        *guard = Some(PreviewServerState {
            serve_dir: serve_dir_str.clone(),
            port: used_port,
        });
    }

    // Spawn daemon thread for the HTTP file server
    let serve_dir_clone = serve_dir.clone();
    std::thread::Builder::new()
        .name("preview-server".to_string())
        .spawn(move || {
            run_file_server(listener, &serve_dir_clone);
        })
        .context("Failed to spawn preview server thread")?;

    let url = build_preview_url(used_port, target_file.as_deref());
    open_browser(&url);

    Ok(format!(
        "Preview server started at {}\n\n\
         Open in browser: {}\n\
         Serving directory: {}\n\
         (Server runs in background. Stops when you exit.)",
        url, url, serve_dir_str
    ))
}

/// Build the preview URL with optional filename.
fn build_preview_url(port: u16, filename: Option<&str>) -> String {
    match filename {
        Some(f) => format!("http://127.0.0.1:{}/{}", port, f),
        None => format!("http://127.0.0.1:{}", port),
    }
}

/// Open a URL in the default browser.
fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "linux") {
        std::process::Command::new("xdg-open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
    } else {
        Ok(std::process::Command::new("true").spawn().unwrap())
    };
}

/// Minimal HTTP file server loop. Handles GET requests, serves static files
/// with no-cache headers. Runs in a daemon thread.
fn run_file_server(listener: std::net::TcpListener, serve_dir: &Path) {
    use std::io::{BufRead, BufReader, Write};

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let reader = BufReader::new(&stream);
        let request_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            _ => continue,
        };

        // Parse "GET /path HTTP/1.x"
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] != "GET" {
            let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\n\r\n");
            continue;
        }

        let request_path = parts[1];
        // Strip query string
        let clean_path = request_path.split('?').next().unwrap_or("/");
        // URL decode %XX sequences (basic)
        let decoded = url_decode(clean_path);
        // Remove leading slash, default to index.html
        let rel = decoded.trim_start_matches('/');
        let rel = if rel.is_empty() { "index.html" } else { rel };

        let file_path = serve_dir.join(rel);
        let normalized = normalize_path(&file_path);

        // Security: path must stay within serve_dir
        if !normalized.starts_with(serve_dir) {
            let body = "403 Forbidden";
            let resp = format!(
                "HTTP/1.1 403 Forbidden\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            continue;
        }

        if normalized.is_file() {
            match std::fs::read(&normalized) {
                Ok(content) => {
                    let mime = guess_mime(&normalized);
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\n\
                         Content-Type: {}\r\n\
                         Content-Length: {}\r\n\
                         Cache-Control: no-store, no-cache, must-revalidate, max-age=0\r\n\
                         Pragma: no-cache\r\n\
                         Connection: close\r\n\r\n",
                        mime,
                        content.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.write_all(&content);
                }
                Err(_) => {
                    let body = "500 Internal Server Error";
                    let resp = format!(
                        "HTTP/1.1 500 Internal Server Error\r\n\
                         Content-Length: {}\r\n\
                         Connection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
        } else {
            let body = "404 Not Found";
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}

/// Basic URL decoding for %XX sequences.
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| hex_val(c));
            let lo = chars.next().and_then(|c| hex_val(c));
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h << 4 | l) as char);
            } else {
                result.push('%');
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Guess MIME type from file extension.
fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") => "text/plain; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("xml") => "application/xml; charset=utf-8",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
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
