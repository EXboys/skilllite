//! agent-rpc JSON-Lines 协议解析与协议级诊断事件（与 `skilllite agent-rpc` 子进程 stdout 对齐）。
//!
//! 同类「可测试契约」还包括桌面侧 prompt 路径白名单：见 [`crate::skilllite_bridge::integrations::prompt_artifact`]。

use serde::Serialize;
use serde_json::json;

pub const MAX_CONSECUTIVE_INVALID_PROTOCOL_LINES: usize = 8;
pub const MAX_TOTAL_INVALID_PROTOCOL_LINES: usize = 20;
pub const INVALID_LINE_PREVIEW_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize)]
pub struct StreamEvent {
    pub event: String,
    pub data: serde_json::Value,
}

pub fn preview_line(line: &str, max_chars: usize) -> String {
    let preview: String = line.chars().take(max_chars).collect();
    if line.chars().count() > max_chars {
        format!("{}...", preview)
    } else {
        preview
    }
}

pub fn parse_stream_event_line(line: &str) -> Result<StreamEvent, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;
    let event = value
        .get("event")
        .and_then(|e| e.as_str())
        .ok_or_else(|| "Protocol error: missing string field 'event'".to_string())?;
    let data = value
        .get("data")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(StreamEvent {
        event: event.to_string(),
        data,
    })
}

pub fn make_protocol_warning_event(
    consecutive_invalid_lines: usize,
    total_invalid_lines: usize,
    line: &str,
    err: &str,
) -> StreamEvent {
    StreamEvent {
        event: "protocol_warning".to_string(),
        data: json!({
            "message": "检测到 agent-rpc 协议流被脏数据污染，正在自动恢复",
            "consecutive_invalid_lines": consecutive_invalid_lines,
            "total_invalid_lines": total_invalid_lines,
            "line_preview": preview_line(line, INVALID_LINE_PREVIEW_CHARS),
            "last_error": err,
        }),
    }
}

pub fn make_protocol_recovered_event(
    recovered_lines: usize,
    total_invalid_lines: usize,
) -> StreamEvent {
    StreamEvent {
        event: "protocol_recovered".to_string(),
        data: json!({
            "message": format!(
                "agent-rpc 协议流已自动恢复，已跳过 {} 行异常输出",
                recovered_lines
            ),
            "recovered_lines": recovered_lines,
            "total_invalid_lines": total_invalid_lines,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        make_protocol_recovered_event, make_protocol_warning_event, parse_stream_event_line,
        preview_line,
    };

    #[test]
    fn parse_stream_event_line_accepts_valid_event() {
        let ev = parse_stream_event_line(r#"{"event":"text","data":{"text":"hello"}}"#)
            .expect("valid JSON-Lines event should parse");
        assert_eq!(ev.event, "text");
        assert_eq!(ev.data["text"], "hello");
    }

    #[test]
    fn parse_stream_event_line_rejects_non_json_noise() {
        let err = parse_stream_event_line("INFO booting agent")
            .expect_err("non-JSON noise should be rejected");
        assert!(err.contains("JSON parse error"));
    }

    #[test]
    fn parse_stream_event_line_rejects_missing_event_field() {
        let err = parse_stream_event_line(r#"{"data":{"text":"hello"}}"#)
            .expect_err("missing event should be rejected");
        assert!(err.contains("missing string field 'event'"));
    }

    #[test]
    fn preview_line_truncates_and_marks_suffix() {
        let preview = preview_line("abcdefghijklmnopqrstuvwxyz", 10);
        assert_eq!(preview, "abcdefghij...");
    }

    #[test]
    fn protocol_warning_event_contains_diagnostic_details() {
        let ev = make_protocol_warning_event(1, 3, "INFO booting agent", "JSON parse error");
        assert_eq!(ev.event, "protocol_warning");
        assert_eq!(ev.data["consecutive_invalid_lines"], 1);
        assert_eq!(ev.data["total_invalid_lines"], 3);
        assert_eq!(ev.data["line_preview"], "INFO booting agent");
    }

    #[test]
    fn protocol_recovered_event_reports_recovery_summary() {
        let ev = make_protocol_recovered_event(2, 5);
        assert_eq!(ev.event, "protocol_recovered");
        assert_eq!(ev.data["recovered_lines"], 2);
        assert_eq!(ev.data["total_invalid_lines"], 5);
    }
}
