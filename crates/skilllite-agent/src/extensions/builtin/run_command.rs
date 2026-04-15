//! run_command: shell command execution with confirmation + timeout.

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use serde_json::{json, Value};
use std::path::Path;
use std::process::ExitStatus;

use crate::high_risk;
use crate::types::{
    safe_slice_from, safe_truncate, ConfirmationRequest, EventSink, FunctionDef, RiskTier,
    ToolDefinition,
};

use super::helpers::filter_sensitive_content_in_text;

/// Hard block for `run_command` when inline shell static scan reports Critical (no override).
const SHELL_SCAN_CRITICAL_BLOCKED: &str =
    "Blocked: shell static scan reported Critical-severity issues that cannot be overridden.";

// ─── Tool definition ────────────────────────────────────────────────────────

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDef {
            name: "run_command".to_string(),
            description: "Execute a shell command in the workspace directory. Uses the platform shell: Unix/macOS runs `sh -c`; Windows runs `%ComSpec% /C` (normally cmd.exe). A static shell scan (same engine family as skill L3 checks) runs before spawn; findings require confirmation. Reading sensitive paths (.env, .key, .pem, .git/config) via shell requires explicit confirmation. Regex-based dangerous patterns (rm -rf, curl|bash, etc.) add warnings. Timeout: 300 seconds.".to_string(),
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

// ─── Blocked vs dangerous command detection ─────────────────────────────────

/// Machine-wide catastrophic patterns: always rejected (even if `SKILLLITE_HIGH_RISK_CONFIRM=none`).
const BLOCKED_PATTERNS: &[(&str, &str)] = &[
    (
        r":\(\)\s*\{\s*:\|:\s*&\s*\}\s*;\s*:",
        "fork bomb — will crash the system",
    ),
    (
        r"(?i)(?:sudo\s+)?rm\s+-[a-zA-Z]*r[a-zA-Z]*\s+/\s*$",
        "rm -rf / — system destruction",
    ),
    (
        r"rm\s+-[a-zA-Z]*r[a-zA-Z]*\s+/\s*\*",
        "rm -rf /* — mass deletion at filesystem root",
    ),
    (
        r"(?i)\bmkfs\.[a-z0-9]+\s+",
        "mkfs on a device — filesystem destruction risk",
    ),
];

// ─── Dangerous command detection (confirm, not hard-deny) ───────────────────

const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (
        r"rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*--force)",
        "rm with force flag — may delete files irreversibly",
    ),
    (
        r"(curl|wget)\s+.*\|\s*(bash|sh|zsh)",
        "piping remote script to shell — remote code execution risk",
    ),
    (
        r"chmod\s+(-[a-zA-Z]*R|--recursive)\s+777",
        "recursive chmod 777 — insecure permission change",
    ),
];

/// Sensitive file reads via shell: always require explicit confirmation (not bypassed by `SKILLLITE_HIGH_RISK_CONFIRM=none`).
const SENSITIVE_FILE_READ_PATTERNS: &[(&str, &str)] = &[
    (
        r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.env",
        "reading .env file",
    ),
    (
        r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.key",
        "reading .key file",
    ),
    (
        r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.pem",
        "reading .pem file",
    ),
    (
        r"(?i)(cat|head|tail|less|more|type|od|xxd|strings)\s+[^\n;|]*\.git/config",
        "reading .git/config",
    ),
    (r"(?i)\.\s+[^\n;|]*\.env\b", "sourcing .env file"),
    (r"(?i)source\s+[^\n;|]*\.env", "sourcing .env file"),
];

fn check_blocked_command(cmd: &str) -> Option<String> {
    for (pattern, reason) in BLOCKED_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(cmd) {
                return Some(reason.to_string());
            }
        }
    }
    // `dd` to a real device (`regex` crate has no look-around; handle `of=/dev/null` explicitly).
    if let Ok(re) = regex::Regex::new(r"(?i)\bdd\s+.*?of=/dev/([^\s;|]+)") {
        if let Some(cap) = re.captures(cmd).and_then(|c| c.get(1)) {
            let tail = cap.as_str();
            if tail != "null" {
                return Some("dd writing to a block device — disk destruction risk".to_string());
            }
        }
    }
    None
}

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
        bail!("command must not be empty");
    }

    if let Some(reason) = check_blocked_command(cmd) {
        bail!(
            "Blocked: command is not allowed ({}). \
             This pattern cannot be executed via run_command.",
            reason
        );
    }

    let shell_scan_note: Option<String> = match skilllite_sandbox::security::scan_shell_command(cmd)
    {
        Ok(scan) => {
            let has_critical = scan.issues.iter().any(|i| {
                matches!(
                    i.severity,
                    skilllite_sandbox::security::SecuritySeverity::Critical
                )
            });
            if has_critical {
                let detail = skilllite_sandbox::security::format_scan_result_compact(&scan);
                bail!("{}\n\n{}", SHELL_SCAN_CRITICAL_BLOCKED, detail);
            }
            if !scan.is_safe {
                Some(skilllite_sandbox::security::format_scan_result_compact(
                    &scan,
                ))
            } else {
                None
            }
        }
        Err(e) => {
            tracing::warn!("run_command shell static scan failed: {}", e);
            Some(format!(
                "Shell static scan failed: {}. Manual review required before running.",
                e
            ))
        }
    };
    let needs_shell_scan_confirm = shell_scan_note.is_some();

    let sensitive_reason = check_sensitive_file_read(cmd);
    let danger_reason = check_dangerous_command(cmd);
    let must_confirm_sensitive = sensitive_reason.is_some();
    let must_confirm_run_command_tool = high_risk::confirm_run_command();

    if must_confirm_sensitive || must_confirm_run_command_tool || needs_shell_scan_confirm {
        let mut confirm_msg = match (&sensitive_reason, &danger_reason) {
            (Some(sr), Some(dr)) => format!(
                "⚠️ Sensitive file access\n\n\
                 This command may expose secrets or credentials ({}).\n\
                 It also matches a risky pattern ({}).\n\n\
                 Command:\n  {}\n\n\
                 Confirm execution?",
                sr, dr, cmd
            ),
            (Some(sr), None) => format!(
                "⚠️ Sensitive file access\n\n\
                 This command may expose secrets or credentials ({}).\n\
                 Paths like .env, .key, .pem, and .git/config require explicit approval when run via run_command.\n\n\
                 Command:\n  {}\n\n\
                 Confirm execution?",
                sr, cmd
            ),
            (None, Some(dr)) => format!(
                "⚠️ Dangerous command detected\n\n\
                 Pattern that may cause serious harm: {}\n\n\
                 Command: {}\n\n\
                 Please verify before confirming execution.",
                dr, cmd
            ),
            (None, None) => format!("About to execute command:\n  {}\n\nConfirm execution?", cmd),
        };

        if let Some(note) = &shell_scan_note {
            confirm_msg.push_str("\n\n—— Shell static scan (pre-spawn) ——\n");
            confirm_msg.push_str(note);
        }

        let tier =
            if sensitive_reason.is_some() || danger_reason.is_some() || needs_shell_scan_confirm {
                RiskTier::ConfirmRequired
            } else {
                RiskTier::Low
            };

        let request = ConfirmationRequest::new(confirm_msg, tier);
        if !event_sink.on_confirmation_request(&request) {
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

    // Do not inherit stdin: GUI / RPC parents often leave stdin open as a pipe with no EOF,
    // which makes `read()` in child scripts (e.g. `sys.stdin.read()`) block until timeout.
    #[cfg(windows)]
    let mut child = {
        use std::os::windows::process::CommandExt;
        let comspec = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        let mut c = Command::new(comspec)
            .arg("/C")
            .arg(cmd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(workspace);
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        c.spawn()
            .with_context(|| format!("Failed to spawn command: {}", cmd))?
    };
    #[cfg(not(windows))]
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(std::process::Stdio::null())
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
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            let _ = child.kill().await;
            event_sink.on_command_finished(false, -1, start_time.elapsed().as_millis() as u64);
            return Ok(build_timeout_outcome());
        }
    };
    let exit_code = status
        .code()
        .unwrap_or(if status.success() { 0 } else { -1 });
    event_sink.on_command_finished(
        status.success(),
        exit_code,
        start_time.elapsed().as_millis() as u64,
    );

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
    let code = status
        .code()
        .unwrap_or(if status.success() { 0 } else { -1 });
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
    parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
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

#[cfg(test)]
mod blocked_command_tests {
    use super::check_blocked_command;
    use super::check_dangerous_command;

    #[test]
    fn fork_bomb_is_blocked() {
        assert!(check_blocked_command(":(){ :|:& };:").is_some());
    }

    #[test]
    fn rm_rf_root_is_blocked() {
        assert!(check_blocked_command("rm -rf /").is_some());
    }

    #[test]
    fn sudo_rm_rf_root_is_blocked() {
        assert!(check_blocked_command("sudo rm -rf /").is_some());
    }

    #[test]
    fn rm_rf_root_glob_is_blocked() {
        assert!(check_blocked_command("rm -rf /*").is_some());
    }

    #[test]
    fn dd_to_block_device_is_blocked() {
        assert!(check_blocked_command("dd if=/dev/zero of=/dev/sda bs=1M").is_some());
    }

    #[test]
    fn dd_to_dev_null_is_not_blocked() {
        assert!(check_blocked_command("dd if=/dev/zero of=/dev/null count=1").is_none());
    }

    #[test]
    fn mkfs_invocation_is_blocked() {
        assert!(check_blocked_command("mkfs.ext4 /dev/sdb1").is_some());
    }

    #[test]
    fn rm_rf_project_is_not_blocked_but_dangerous() {
        assert!(check_blocked_command("rm -rf ./dist").is_none());
        assert!(check_dangerous_command("rm -rf ./dist").is_some());
    }
}
