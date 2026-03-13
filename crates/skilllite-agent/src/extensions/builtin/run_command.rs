//! run_command: shell command execution with confirmation + timeout.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitStatus;

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
) -> Result<RunCommandOutcome> {
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
            return Ok(RunCommandOutcome {
                content: "User cancelled command execution".to_string(),
                is_error: false,
                counts_as_failure: false,
            });
        }
    }

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;
    use tokio::sync::mpsc;
    let start_time = std::time::Instant::now();

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(workspace)
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;
    event_sink.on_command_started(cmd);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, mut rx) = mpsc::unbounded_channel::<(&'static str, String)>();

    if let Some(stdout) = stdout {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(("stdout", line)).is_err() {
                    break;
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx.send(("stderr", line)).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    let timeout_duration = tokio::time::Duration::from_secs(300);
    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();
    let mut redacted_any = false;

    let status = match tokio::time::timeout(timeout_duration, async {
        let mut wait_fut = Box::pin(child.wait());
        let mut status: Option<ExitStatus> = None;
        let mut streams_open = true;

        while status.is_none() || streams_open {
            tokio::select! {
                maybe = rx.recv(), if streams_open => {
                    match maybe {
                        Some((stream, line)) => {
                            let (filtered_line, redacted) = filter_sensitive_content_in_text(&line);
                            if redacted {
                                redacted_any = true;
                            }
                            event_sink.on_command_output(stream, &filtered_line);
                            if stream == "stderr" {
                                stderr_lines.push(filtered_line);
                            } else {
                                stdout_lines.push(filtered_line);
                            }
                        }
                        None => streams_open = false,
                    }
                }
                wait_res = &mut wait_fut, if status.is_none() => {
                    status = Some(wait_res.with_context(|| format!("Error waiting for command: {}", cmd))?);
                }
            }
        }

        status.context("command finished without exit status")
    }).await {
        Ok(Ok(status)) => status,
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            let _ = child.kill().await;
            event_sink.on_command_finished(false, -1, start_time.elapsed().as_millis() as u64);
            return Ok(build_timeout_outcome());
        }
    };
    let exit_code = status.code().unwrap_or(if status.success() { 0 } else { -1 });
    event_sink.on_command_finished(status.success(), exit_code, start_time.elapsed().as_millis() as u64);

    let stdout_text = stdout_lines.join("\n");
    let stderr_text = stderr_lines.join("\n");
    Ok(build_command_result(
        status,
        &stdout_text,
        &stderr_text,
        redacted_any,
    ))
}

const MAX_COMMAND_RESULT_CHARS: usize = 2000;

#[derive(Debug, Clone)]
pub(super) struct RunCommandOutcome {
    pub content: String,
    pub is_error: bool,
    pub counts_as_failure: bool,
}

fn build_command_result(
    status: ExitStatus,
    stdout_text: &str,
    stderr_text: &str,
    redacted: bool,
) -> RunCommandOutcome {
    let code = status.code().unwrap_or(if status.success() { 0 } else { -1 });
    let mut result = if status.success() {
        format!("Command succeeded (exit {}).", code)
    } else {
        format!("Command failed (exit {}).", code)
    };

    if stdout_text.is_empty() && stderr_text.is_empty() {
        result.push_str("\nNo stdout/stderr was produced.");
    } else {
        result.push_str("\nOutput streamed to execution log.");
        let preview = build_output_preview(status.success(), stdout_text, stderr_text);
        if !preview.is_empty() {
            result.push_str("\n\nPreview:\n");
            result.push_str(&truncate_command_output(&preview));
        }
    }

    if redacted {
        result.push_str("\n\n[⚠️ Sensitive values (API_KEY, PASSWORD, etc.) have been redacted]");
    }

    RunCommandOutcome {
        content: result,
        is_error: !status.success(),
        counts_as_failure: false,
    }
}

fn build_timeout_outcome() -> RunCommandOutcome {
    RunCommandOutcome {
        content: "Error: Command execution timeout (300s)".to_string(),
        is_error: true,
        counts_as_failure: true,
    }
}

#[cfg(test)]
pub(super) fn timeout_outcome_for_test() -> RunCommandOutcome {
    build_timeout_outcome()
}

fn build_output_preview(success: bool, stdout_text: &str, stderr_text: &str) -> String {
    let mut parts = Vec::new();
    if success {
        if !stderr_text.is_empty() {
            parts.push(build_preview_block("stderr", stderr_text, 3, true));
        } else if !stdout_text.is_empty() {
            parts.push(build_preview_block("stdout tail", stdout_text, 2, true));
        }
    } else {
        if !stderr_text.is_empty() {
            parts.push(build_preview_block("stderr", stderr_text, 6, false));
        }
        if !stdout_text.is_empty() {
            parts.push(build_preview_block("stdout tail", stdout_text, 3, true));
        }
    }
    parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n")
}

fn build_preview_block(label: &str, text: &str, max_lines: usize, prefer_tail: bool) -> String {
    let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    if lines.is_empty() {
        return String::new();
    }

    let start = if prefer_tail {
        lines.len().saturating_sub(max_lines)
    } else {
        0
    };
    let shown = &lines[start..];
    let mut block = format!("[{}]\n{}", label, shown.join("\n"));
    let hidden_count = lines.len().saturating_sub(shown.len());
    if hidden_count > 0 {
        if prefer_tail {
            block.push_str(&format!("\n... {} earlier lines streamed", hidden_count));
        } else {
            block.push_str(&format!("\n... {} more lines streamed", hidden_count));
        }
    }
    block
}

#[cfg(test)]
pub(super) fn truncate_command_output_for_test(output: &str) -> String {
    truncate_command_output(output)
}

fn truncate_command_output(output: &str) -> String {
    if output.len() <= MAX_COMMAND_RESULT_CHARS {
        return output.to_string();
    }

    let head_size = MAX_COMMAND_RESULT_CHARS * 2 / 3;
    let tail_size = MAX_COMMAND_RESULT_CHARS / 3;
    let head = safe_truncate(output, head_size);
    let tail = safe_slice_from(output, output.len().saturating_sub(tail_size));

    format!(
        "{}\n\n[... preview truncated: {} total chars, showing head + tail ...]\n\n{}",
        head,
        output.len(),
        tail
    )
}
