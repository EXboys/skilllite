//! run_command: shell command execution with confirmation + timeout.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;

use crate::high_risk;
use crate::types::{EventSink, ToolDefinition, FunctionDef, safe_truncate, safe_slice_from};

use super::helpers::filter_sensitive_content_in_text;

// ─── Tool definition ────────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDef {
            name: "run_command".to_string(),
            description: "Execute a shell command in the workspace directory. Requires user confirmation before execution. Blocks reading sensitive files (cat .env, cat .key, etc.). Dangerous commands (rm -rf, curl|bash, etc.) are flagged with extra warnings. Timeout: 300 seconds.".to_string(),
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
    }]
}

// ─── Dangerous command detection ────────────────────────────────────────────

const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*--force)", "rm with force flag — may delete files irreversibly"),
    (r"rm\s+-[a-zA-Z]*r[a-zA-Z]*\s+/\s*$", "rm -rf / — system destruction"),
    (r"(curl|wget)\s+.*\|\s*(bash|sh|zsh)", "piping remote script to shell — remote code execution risk"),
    (r":\(\)\s*\{\s*:\|:\s*&\s*\}\s*;\s*:", "fork bomb — will crash the system"),
    (r"chmod\s+(-[a-zA-Z]*R|--recursive)\s+777", "recursive chmod 777 — insecure permission change"),
];

/// 检测是否尝试读取敏感文件（cat .env 等），直接 block 不可绕过
const SENSITIVE_FILE_READ_PATTERNS: &[(&str, &str)] = &[
    (r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.env", "reading .env file"),
    (r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.key", "reading .key file"),
    (r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.pem", "reading .pem file"),
    (r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.git/config", "reading .git/config"),
    (r"(?i)\.\s+[^\n;|]*\.env\b", "sourcing .env file"),
    (r"(?i)source\s+[^\n;|]*\.env", "sourcing .env file"),
];

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

fn check_sensitive_file_read(cmd: &str) -> Option<String> {
    for (pattern, reason) in SENSITIVE_FILE_READ_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(cmd) {
                return Some(reason.to_string());
            }
        }
    }
    None
}

// ─── Execution ──────────────────────────────────────────────────────────────

pub(super) async fn execute_run_command(
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

    // A11: 禁止通过 run_command 绕过敏感文件读取（cat .env 等直接 block）
    if let Some(reason) = check_sensitive_file_read(cmd) {
        anyhow::bail!(
            "Blocked: command attempts to read sensitive file ({}). \
             .env, .key, .git/config, .pem cannot be read via run_command.",
            reason
        );
    }

    // A11: run_command 可配置为跳过确认
    if high_risk::confirm_run_command() {
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

        if !event_sink.on_confirmation_request(&confirm_msg) {
            return Ok("User cancelled command execution".to_string());
        }
    }

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

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut output_lines = Vec::new();

    if let Some(stdout) = stdout {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            output_lines.push(line);
        }
    }

    let mut stderr_lines = Vec::new();
    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            stderr_lines.push(line);
        }
    }

    let timeout_duration = tokio::time::Duration::from_secs(300);
    let status = match tokio::time::timeout(timeout_duration, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => {
            return Ok(format!("Error waiting for command: {}", e));
        }
        Err(_) => {
            let _ = child.kill().await;
            return Ok("Error: Command execution timeout (300s)".to_string());
        }
    };

    let stdout_text = output_lines.join("\n");
    let stderr_text = stderr_lines.join("\n");
    let mut result = String::new();

    if status.success() {
        if stdout_text.is_empty() && stderr_text.is_empty() {
            result.push_str("Command succeeded (exit 0)");
        } else {
            let (filtered_stdout, redacted) = filter_sensitive_content_in_text(&stdout_text);
            let filtered_stderr = if !stderr_text.is_empty() {
                filter_sensitive_content_in_text(&stderr_text).0
            } else {
                String::new()
            };
            result.push_str(&format!("Command succeeded (exit 0):\n{}", filtered_stdout));
            if !filtered_stderr.is_empty() {
                result.push_str(&format!("\n[stderr]: {}", filtered_stderr));
            }
            if redacted {
                result.push_str("\n\n[⚠️ Sensitive values (API_KEY, PASSWORD, etc.) have been redacted]");
            }
        }
    } else {
        let code = status.code().unwrap_or(-1);
        let (combined, redacted) = if !stdout_text.is_empty() && !stderr_text.is_empty() {
            let (f_stdout, r1) = filter_sensitive_content_in_text(&stdout_text);
            let (f_stderr, r2) = filter_sensitive_content_in_text(&stderr_text);
            (format!("{}\n[stderr]: {}", f_stdout, f_stderr), r1 || r2)
        } else if !stderr_text.is_empty() {
            let (f, r) = filter_sensitive_content_in_text(&stderr_text);
            (f, r)
        } else {
            let (f, r) = filter_sensitive_content_in_text(&stdout_text);
            (f, r)
        };
        result.push_str(&format!("Command failed (exit {}):\n{}", code, combined));
        if redacted {
            result.push_str("\n\n[⚠️ Sensitive values (API_KEY, PASSWORD, etc.) have been redacted]");
        }
    }

    Ok(truncate_command_output(&result))
}

const MAX_COMMAND_OUTPUT_CHARS: usize = 8000;

#[cfg(test)]
pub(super) fn truncate_command_output_for_test(output: &str) -> String {
    truncate_command_output(output)
}

fn truncate_command_output(output: &str) -> String {
    if output.len() <= MAX_COMMAND_OUTPUT_CHARS {
        return output.to_string();
    }

    let head_size = MAX_COMMAND_OUTPUT_CHARS * 2 / 3;
    let tail_size = MAX_COMMAND_OUTPUT_CHARS / 3;
    let head = safe_truncate(output, head_size);
    let tail = safe_slice_from(output, output.len().saturating_sub(tail_size));

    format!(
        "{}\n\n[... output truncated: {} total chars, showing head + tail ...]\n\n{}",
        head,
        output.len(),
        tail
    )
}
