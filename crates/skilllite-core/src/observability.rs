//! Observability: tracing init, audit log, security events.
//!
//! Uses config::ObservabilityConfig for SKILLLITE_QUIET, LOG_LEVEL, AUDIT_LOG, etc.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use serde_json::json;
use tracing_subscriber::{prelude::*, EnvFilter};
use uuid::Uuid;

static SECURITY_EVENTS_PATH: Mutex<Option<String>> = Mutex::new(None);

/// Tracing initialization mode.
#[derive(Clone, Copy)]
pub enum TracingMode {
    /// Default: use SKILLLITE_LOG_LEVEL / SKILLLITE_QUIET from env
    Default,
    /// Chat: suppress agent-internal WARN (compaction, task planning) to keep UI clean
    Chat,
}

/// Initialize tracing. Call at process startup.
/// When SKILLLITE_QUIET=1 (or SKILLBOX_QUIET for compat), only WARN and above are logged.
pub fn init_tracing(mode: TracingMode) {
    let cfg = crate::config::ObservabilityConfig::from_env();
    let mut level: String = if cfg.quiet {
        "skilllite=warn".to_string()
    } else {
        cfg.log_level.clone()
    };

    // Chat mode: suppress agent-internal warnings (compaction, task planning) to avoid polluting the UI
    if matches!(mode, TracingMode::Chat) {
        level = format!("{},skilllite::agent=error", level);
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&level));

    let json = cfg.log_json;

    let _ = if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(true)
                    .with_thread_ids(false),
            )
            .try_init()
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false),
            )
            .try_init()
    };
}

/// 解析审计日志实际写入路径。目录则按天存储 audit_YYYY-MM-DD.jsonl；.jsonl 文件则直接写入。
fn get_audit_path() -> Option<String> {
    let base = crate::config::ObservabilityConfig::from_env()
        .audit_log
        .clone()?;
    if base.is_empty() {
        return None;
    }
    let path = Path::new(&base);
    let file_path = if base.ends_with(".jsonl") {
        path.to_path_buf()
    } else {
        let today = chrono::Utc::now().format("%Y-%m-%d");
        path.join(format!("audit_{}.jsonl", today))
    };
    let file_path_str = file_path.to_string_lossy().into_owned();
    if let Some(parent) = file_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    Some(file_path_str)
}

fn get_security_events_path() -> Option<String> {
    {
        let guard = SECURITY_EVENTS_PATH.lock().ok()?;
        if let Some(ref p) = *guard {
            return Some(p.clone());
        }
    }
    let path = crate::config::ObservabilityConfig::from_env()
        .security_events_log
        .clone()?;
    if path.is_empty() {
        return None;
    }
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    {
        let mut guard = SECURITY_EVENTS_PATH.lock().ok()?;
        *guard = Some(path.clone());
    }
    Some(path)
}

fn append_jsonl(path: &str, record: &serde_json::Value) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_string(record) {
            let _ = writeln!(f, "{}", line);
            let _ = f.flush(); // 确保每条记录单独落盘，避免流式消费时行粘连
        }
    }
}

/// Audit: confirmation_requested (Rust-side L3 scan)
pub fn audit_confirmation_requested(
    skill_id: &str,
    code_hash: &str,
    issues_count: usize,
    severity: &str,
) {
    if let Some(path) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "confirmation_requested",
            "skill_id": skill_id,
            "code_hash": code_hash,
            "issues_count": issues_count,
            "severity": severity,
            "source": "rust"
        });
        append_jsonl(&path, &record);
    }
}

/// Audit: confirmation_response (Rust-side user/auto)
pub fn audit_confirmation_response(skill_id: &str, approved: bool, source: &str) {
    if let Some(path) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "confirmation_response",
            "skill_id": skill_id,
            "approved": approved,
            "source": source,
            "source_layer": "rust"
        });
        append_jsonl(&path, &record);
    }
}

/// Audit: execution_started (right before spawn — Python name: execution_started)
///
/// Also emits as "command_invoked" for backward compatibility.
pub fn audit_execution_started(skill_id: &str, cmd: &str, args: &[&str], cwd: &str) {
    if let Some(path) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "execution_started",
            "skill_id": skill_id,
            "cmd": cmd,
            "args": args,
            "cwd": cwd,
            "source": "rust"
        });
        append_jsonl(&path, &record);
    }
}

/// Audit: command_invoked — alias for execution_started (backward compat)
pub fn audit_command_invoked(skill_id: &str, cmd: &str, args: &[&str], cwd: &str) {
    audit_execution_started(skill_id, cmd, args, cwd);
}

/// Audit: execution_completed (Rust-side)
pub fn audit_execution_completed(
    skill_id: &str,
    exit_code: i32,
    duration_ms: u64,
    stdout_len: usize,
) {
    if let Some(path) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "execution_completed",
            "skill_id": skill_id,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "stdout_len": stdout_len,
            "success": exit_code == 0,
            "source": "rust"
        });
        append_jsonl(&path, &record);
    }
}

/// Audit: skill_invocation (P0 可观测 - 记录谁在什么上下文调用了哪个 Skill、输入摘要、输出摘要)
pub fn audit_skill_invocation(
    skill_id: &str,
    entry_point: &str,
    cwd: &str,
    input_json: &str,
    output: &str,
    exit_code: i32,
    duration_ms: u64,
) {
    if let Some(path) = get_audit_path() {
        let context = crate::config::loader::env_optional(
            crate::config::env_keys::observability::SKILLLITE_AUDIT_CONTEXT,
            &[],
        )
        .unwrap_or_else(|| "cli".to_string());
        let input_summary = input_summary_bytes(input_json);
        let output_summary = output_summary_bytes(output);
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "skill_invocation",
            "skill_id": skill_id,
            "entry_point": entry_point,
            "cwd": cwd,
            "context": context,
            "input_summary": input_summary,
            "output_summary": output_summary,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "success": exit_code == 0,
            "source": "rust"
        });
        append_jsonl(&path, &record);
    }
}

fn input_summary_bytes(input: &str) -> serde_json::Value {
    let (redacted, preview_redacted) =
        crate::audit_preview_redact::redact_audit_preview_text(input);
    let preview: String = redacted.chars().take(100).collect();
    let truncated = redacted.chars().count() > 100;
    serde_json::json!({
        "len": input.len(),
        "preview": if truncated { format!("{}...", preview) } else { preview },
        "preview_redacted": preview_redacted
    })
}

fn output_summary_bytes(output: &str) -> serde_json::Value {
    let (redacted, preview_redacted) =
        crate::audit_preview_redact::redact_audit_preview_text(output);
    let preview: String = redacted.chars().take(100).collect();
    let truncated = redacted.chars().count() > 100;
    serde_json::json!({
        "len": output.len(),
        "preview": if truncated { format!("{}...", preview) } else { preview },
        "preview_redacted": preview_redacted
    })
}

/// Security event: network blocked
pub fn security_blocked_network(skill_id: &str, blocked_target: &str, reason: &str) {
    tracing::warn!(
        skill_id = %skill_id,
        blocked_target = %blocked_target,
        reason = %reason,
        "Security: blocked network request"
    );
    if let Some(path) = get_security_events_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "security_blocked",
            "category": "network",
            "skill_id": skill_id,
            "details": {
                "blocked_target": blocked_target,
                "reason": reason
            }
        });
        append_jsonl(&path, &record);
    }
}

/// Security event: scan found high/critical
pub fn security_scan_high(skill_id: &str, severity: &str, issues: &serde_json::Value) {
    if let Some(path) = get_security_events_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "security_scan_high",
            "category": "code_scan",
            "skill_id": skill_id,
            "details": {
                "severity": severity,
                "issues": issues
            }
        });
        append_jsonl(&path, &record);
    }
}

/// Security event: scan approved — user approved after high/critical scan
pub fn security_scan_approved(skill_id: &str, scan_id: &str, issues_count: usize) {
    tracing::info!(
        skill_id = %skill_id,
        scan_id = %scan_id,
        issues_count = %issues_count,
        "Security: scan approved by user"
    );
    if let Some(path) = get_security_events_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "security_scan_approved",
            "category": "code_scan",
            "skill_id": skill_id,
            "details": {
                "scan_id": scan_id,
                "issues_count": issues_count,
                "decision": "approved"
            }
        });
        append_jsonl(&path, &record);
    }
}

/// Security event: scan rejected — user rejected after high/critical scan
pub fn security_scan_rejected(skill_id: &str, scan_id: &str, issues_count: usize) {
    tracing::info!(
        skill_id = %skill_id,
        scan_id = %scan_id,
        issues_count = %issues_count,
        "Security: scan rejected by user"
    );
    if let Some(path) = get_security_events_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "security_scan_rejected",
            "category": "code_scan",
            "skill_id": skill_id,
            "details": {
                "scan_id": scan_id,
                "issues_count": issues_count,
                "decision": "rejected"
            }
        });
        append_jsonl(&path, &record);
    }
}

// ─── Edit audit events (agent layer) ────────────────────────────────────────
//
// 结构约定：
// - path 提升到顶层，便于查询
// - edit_id 每条唯一，用于去重与关联
// - workspace/context 可选，用于多项目过滤

fn edit_audit_context() -> serde_json::Value {
    crate::config::loader::env_optional(
        crate::config::env_keys::observability::SKILLLITE_AUDIT_CONTEXT,
        &[],
    )
    .map(serde_json::Value::String)
    .unwrap_or(serde_json::Value::Null)
}

/// Audit: edit_applied — agent wrote a file change via search_replace
pub fn audit_edit_applied(
    path: &str,
    occurrences: usize,
    first_changed_line: usize,
    diff_excerpt: &str,
    workspace: Option<&str>,
) {
    if let Some(audit) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "edit_applied",
            "category": "edit",
            "source_layer": "agent",
            "edit_id": Uuid::new_v4().to_string(),
            "path": path,
            "workspace": workspace.unwrap_or(""),
            "context": edit_audit_context(),
            "details": {
                "occurrences": occurrences,
                "first_changed_line": first_changed_line,
                "diff_excerpt": diff_excerpt
            }
        });
        append_jsonl(&audit, &record);
    }
}

/// Audit: edit_previewed — agent computed a dry-run diff via preview_edit
pub fn audit_edit_previewed(
    path: &str,
    occurrences: usize,
    first_changed_line: usize,
    diff_excerpt: &str,
    workspace: Option<&str>,
) {
    if let Some(audit) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "edit_previewed",
            "category": "edit",
            "source_layer": "agent",
            "edit_id": Uuid::new_v4().to_string(),
            "path": path,
            "workspace": workspace.unwrap_or(""),
            "context": edit_audit_context(),
            "details": {
                "occurrences": occurrences,
                "first_changed_line": first_changed_line,
                "diff_excerpt": diff_excerpt
            }
        });
        append_jsonl(&audit, &record);
    }
}

/// Audit: edit_inserted — agent inserted lines via insert_lines
pub fn audit_edit_inserted(
    path: &str,
    line_num: usize,
    lines_inserted: usize,
    diff_excerpt: &str,
    workspace: Option<&str>,
) {
    if let Some(audit) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "edit_inserted",
            "category": "edit",
            "source_layer": "agent",
            "edit_id": Uuid::new_v4().to_string(),
            "path": path,
            "workspace": workspace.unwrap_or(""),
            "context": edit_audit_context(),
            "details": {
                "insert_after_line": line_num,
                "lines_inserted": lines_inserted,
                "diff_excerpt": diff_excerpt
            }
        });
        append_jsonl(&audit, &record);
    }
}

/// Audit: edit_failed — agent attempted an edit that failed (not found, non-unique, etc.)
pub fn audit_edit_failed(path: &str, tool_name: &str, reason: &str, workspace: Option<&str>) {
    if let Some(audit) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "edit_failed",
            "category": "edit",
            "source_layer": "agent",
            "edit_id": Uuid::new_v4().to_string(),
            "path": path,
            "reason": reason,
            "tool": tool_name,
            "workspace": workspace.unwrap_or(""),
            "context": edit_audit_context(),
            "details": {
                "path": path,
                "tool": tool_name,
                "reason": reason
            }
        });
        append_jsonl(&audit, &record);
    }
}

// ─── Security events ────────────────────────────────────────────────────────

// ─── Evolution audit events (EVO-5) ─────────────────────────────────────────

/// Audit: evolution event — logged when evolution produces changes or rolls back.
pub fn audit_evolution_event(event_type: &str, target_id: &str, reason: &str, txn_id: &str) {
    if let Some(path) = get_audit_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "event": "evolution",
            "category": "evolution",
            "source_layer": "agent",
            "details": {
                "type": event_type,
                "target_id": target_id,
                "reason": reason,
                "txn_id": txn_id
            }
        });
        append_jsonl(&path, &record);
    }
}

/// Security event: sandbox fallback (e.g. Seatbelt failed, using simple execution)
pub fn security_sandbox_fallback(skill_id: &str, reason: &str) {
    tracing::warn!(
        skill_id = %skill_id,
        reason = %reason,
        "Security: sandbox fallback to simple execution"
    );
    if let Some(path) = get_security_events_path() {
        let record = json!({
            "ts": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "type": "sandbox_fallback",
            "category": "runtime",
            "skill_id": skill_id,
            "details": { "reason": reason }
        });
        append_jsonl(&path, &record);
    }
}

#[cfg(test)]
mod summary_tests {
    use serde_json::Value;

    #[test]
    fn input_summary_redacts_secret_before_preview() {
        let input = r#"{"api_key":"sk-abcdefghijklmnopqrstuvwxyz0123456789"}"#;
        let v = super::input_summary_bytes(input);
        let obj = v.as_object().expect("object");
        assert_eq!(
            obj.get("len").and_then(Value::as_u64),
            Some(input.len() as u64)
        );
        assert_eq!(
            obj.get("preview_redacted").and_then(Value::as_bool),
            Some(true)
        );
        let preview = obj.get("preview").and_then(Value::as_str).expect("preview");
        assert!(!preview.contains("abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(preview.contains("REDACTED"));
    }

    #[test]
    fn output_summary_benign_no_redacted_flag_false() {
        let out = r#"{"ok":true}"#;
        let v = super::output_summary_bytes(out);
        let obj = v.as_object().expect("object");
        assert_eq!(
            obj.get("preview_redacted").and_then(Value::as_bool),
            Some(false)
        );
    }
}
