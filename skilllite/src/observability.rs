//! Observability: tracing init, audit log, security events.
//!
//! Uses config::ObservabilityConfig for SKILLLITE_QUIET, LOG_LEVEL, AUDIT_LOG, etc.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use serde_json::json;
use tracing_subscriber::{EnvFilter, prelude::*};

static AUDIT_PATH: Mutex<Option<String>> = Mutex::new(None);
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

fn get_audit_path() -> Option<String> {
    {
        let guard = AUDIT_PATH.lock().ok()?;
        if let Some(ref p) = *guard {
            return Some(p.clone());
        }
    }
    let path = crate::config::ObservabilityConfig::from_env().audit_log.clone()?;
    if path.is_empty() {
        return None;
    }
    // Ensure parent dir exists
    if let Some(parent) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    {
        let mut guard = AUDIT_PATH.lock().ok()?;
        *guard = Some(path.clone());
    }
    Some(path)
}

fn get_security_events_path() -> Option<String> {
    {
        let guard = SECURITY_EVENTS_PATH.lock().ok()?;
        if let Some(ref p) = *guard {
            return Some(p.clone());
        }
    }
    let path = crate::config::ObservabilityConfig::from_env().security_events_log.clone()?;
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
