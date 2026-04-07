//! Redact likely secrets before writing audit log previews (skill I/O summaries).
//!
//! Aligns with `skilllite-agent` `filter_sensitive_content_in_text` key list and patterns;
//! kept in core so `observability` does not depend on upper layers.

use std::sync::LazyLock;

use regex::Regex;

/// Lowercase key names (normalized: `-` → `_`) matched for `KEY=value` and JSON `"key": "..."`.
const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "api-key",
    "password",
    "passwd",
    "pwd",
    "secret",
    "secret_key",
    "secretkey",
    "token",
    "access_token",
    "refresh_token",
    "credential",
    "credentials",
    "private_key",
    "privatekey",
    "access_key",
    "accesskey",
    "auth",
    "authorization",
];

static RE_SK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-[a-zA-Z0-9]{20,}").expect("RE_SK pattern is valid"));

static RE_BEARER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)Bearer\s+[a-zA-Z0-9._-]{20,}").expect("RE_BEARER pattern is valid")
});

/// Full-string redaction for audit previews. Returns `(redacted_text, any_redaction)`.
pub fn redact_audit_preview_text(content: &str) -> (String, bool) {
    let mut out = String::with_capacity(content.len());
    let mut redacted = false;

    for line in content.lines() {
        let (filtered, r) = filter_line_sensitive(line);
        if r {
            redacted = true;
        }
        out.push_str(&filtered);
        out.push('\n');
    }
    if !content.ends_with('\n') && !out.is_empty() {
        out.pop();
    }

    let before = out.clone();
    out = redact_api_key_patterns(&out);
    if out != before {
        redacted = true;
    }

    (out, redacted)
}

fn filter_line_sensitive(line: &str) -> (String, bool) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return (line.to_string(), false);
    }

    let mut out = line.to_string();
    let mut redacted = false;

    if let Some(eq) = trimmed.find('=') {
        let key = trimmed[..eq].trim().to_lowercase().replace('-', "_");
        let key_clean: String = key
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if SENSITIVE_KEYS
            .iter()
            .any(|k| key_clean == *k || key_clean.ends_with(k))
        {
            if let Some(pos) = out.find('=') {
                out = format!("{}[REDACTED]", &out[..=pos]);
                redacted = true;
            }
        }
    }

    for k in SENSITIVE_KEYS {
        let pat = format!(r#""{}"\s*:\s*"[^"]*""#, k);
        if let Ok(re) = Regex::new(&pat) {
            if re.is_match(&out) {
                out = re
                    .replace_all(&out, format!(r#""{}": "[REDACTED]""#, k))
                    .to_string();
                redacted = true;
            }
        }
    }

    (out, redacted)
}

fn redact_api_key_patterns(s: &str) -> String {
    let mut out = RE_SK.replace_all(s, "sk-[REDACTED]").to_string();
    out = RE_BEARER.replace_all(&out, "Bearer [REDACTED]").to_string();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openai_style_key_in_preview_window() {
        let s = r#"{"model":"gpt-4","api_key":"sk-123456789012345678901234567890"}"#;
        let (out, r) = redact_audit_preview_text(s);
        assert!(r);
        assert!(!out.contains("sk-123456789012345678901234567890"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_bearer_token() {
        let s = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz0123456789";
        let (out, r) = redact_audit_preview_text(s);
        assert!(r);
        assert!(out.contains("Bearer [REDACTED]"));
        assert!(!out.contains("abcdefghijklmnopqrstuvwxyz0123456789"));
    }

    #[test]
    fn redacts_env_style_assignment() {
        let s = "OPENAI_API_KEY=supersecretvaluehere";
        let (out, r) = redact_audit_preview_text(s);
        assert!(r);
        assert!(out.contains("=[REDACTED]"));
        assert!(!out.contains("supersecretvaluehere"));
    }

    #[test]
    fn unicode_line_redact_does_not_panic_and_preserves_structure() {
        let s = r#"{"msg":"说明","token":"密钥🔑混合值"}"#;
        let (out, r) = redact_audit_preview_text(s);
        assert!(r);
        assert!(out.contains("说明"));
        assert!(!out.contains("密钥🔑混合值"));
        assert!(out.contains(r#""token": "[REDACTED]""#));
    }

    #[test]
    fn benign_json_unchanged_flag_false() {
        let s = r#"{"hello":"world","count":3}"#;
        let (out, r) = redact_audit_preview_text(s);
        assert!(!r);
        assert_eq!(out, s);
    }
}
