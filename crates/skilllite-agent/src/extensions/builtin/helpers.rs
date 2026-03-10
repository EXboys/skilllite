//! Shared helpers for the builtin tools module.
//!
//! Security validation, path resolution, directory listing, and truncated JSON recovery.

use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::types;

// ─── Security helpers (shared by submodules via super::) ─────────────────────

const SENSITIVE_PATTERNS: &[&str] = &[".env", ".git/config", ".key"];

/// A11: 关键路径 — 需要确认但非完全禁止（如 package.json、Cargo.toml、配置文件等）
const KEY_PATH_PATTERNS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.toml",
    "Cargo.lock",
    "requirements.txt",
    "pyproject.toml",
    "Pipfile",
    "tsconfig.json",
    "jsconfig.json",
    "vite.config.",
    "webpack.config.",
    ".config.",
    "dockerfile",
    "Dockerfile",
    "Makefile",
];

pub(super) fn is_sensitive_write_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.ends_with(pattern) || lower.contains(&format!("{}/", pattern)) {
            return true;
        }
    }
    if lower.ends_with(".key") || lower.ends_with(".pem") {
        return true;
    }
    false
}

/// 敏感路径（读操作复用与写相同的模式）
pub(super) fn is_sensitive_read_path(path: &str) -> bool {
    is_sensitive_write_path(path)
}

/// 其他文件中需脱敏的 key（KEY=value 或 "key": "value" 格式，小写匹配）
const SENSITIVE_KEYS: &[&str] = &[
    "api_key", "apikey", "api-key",
    "password", "passwd", "pwd",
    "secret", "secret_key", "secretkey",
    "token", "access_token", "refresh_token",
    "credential", "credentials",
    "private_key", "privatekey",
    "access_key", "accesskey",
    "auth", "authorization",
];

/// 对任意内容做敏感信息过滤（用于 read_file、run_command 等）
pub(crate) fn filter_sensitive_content_in_text(content: &str) -> (String, bool) {
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

    // 脱敏 API key 等格式：sk-xxx, Bearer xxx
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

    // KEY=value 格式
    if let Some(eq) = trimmed.find('=') {
        let key = trimmed[..eq].trim().to_lowercase().replace('-', "_");
        let key_clean: String = key.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
        if SENSITIVE_KEYS.iter().any(|k| key_clean == *k || key_clean.ends_with(k)) {
            if let Some(pos) = out.find('=') {
                out = format!("{}[REDACTED]", &out[..=pos]);
                redacted = true;
            }
        }
    }

    // JSON "key": "value" 格式（一行可能有多处）
    for k in SENSITIVE_KEYS {
        let pat = format!(r#""{}"\s*:\s*"[^"]*""#, k);
        if let Ok(re) = regex::Regex::new(&pat) {
            if re.is_match(&out) {
                out = re.replace_all(&out, format!(r#""{}": "[REDACTED]""#, k)).to_string();
                redacted = true;
            }
        }
    }

    (out, redacted)
}

fn redact_api_key_patterns(s: &str) -> String {
    let mut out = s.to_string();
    if let Ok(re) = regex::Regex::new(r"sk-[a-zA-Z0-9]{20,}") {
        out = re.replace_all(&out, "sk-[REDACTED]").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"(?i)Bearer\s+[a-zA-Z0-9._-]{20,}") {
        out = re.replace_all(&out, "Bearer [REDACTED]").to_string();
    }
    out
}

/// A11: 是否为关键路径（需确认，非敏感路径直接 block）
pub(super) fn is_key_write_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_lowercase();
    let basename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    for pattern in KEY_PATH_PATTERNS {
        if lower.ends_with(pattern)
            || lower.contains(&format!("/{}", pattern))
            || basename == *pattern
            || basename.starts_with(pattern)
        {
            return true;
        }
    }
    false
}

pub(super) fn resolve_within_workspace(path: &str, workspace: &Path) -> Result<PathBuf> {
    let input = Path::new(path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        workspace.join(input)
    };

    let normalized = normalize_path(&resolved);

    if !normalized.starts_with(workspace) {
        let is_output_path = types::get_output_dir()
            .map_or(false, |od| normalized.starts_with(Path::new(&od)));
        if is_output_path {
            anyhow::bail!(
                "Path escapes workspace: {} (workspace: {}). \
                 Hint: this path is in the output directory — use **write_output** \
                 (with file_path relative to the output dir) instead of write_file.",
                path,
                workspace.display()
            );
        } else {
            anyhow::bail!(
                "Path escapes workspace: {} (workspace: {})",
                path,
                workspace.display()
            );
        }
    }

    Ok(normalized)
}

pub(super) fn resolve_within_workspace_or_output(path: &str, workspace: &Path) -> Result<PathBuf> {
    if let Ok(resolved) = resolve_within_workspace(path, workspace) {
        return Ok(resolved);
    }

    if let Some(output_dir) = types::get_output_dir() {
        let output_root = PathBuf::from(&output_dir);
        let input = Path::new(path);
        let resolved = if input.is_absolute() {
            input.to_path_buf()
        } else {
            output_root.join(input)
        };
        let normalized = normalize_path(&resolved);
        if normalized.starts_with(&output_root) {
            return Ok(normalized);
        }
    }

    anyhow::bail!(
        "Path escapes workspace: {} (workspace: {})",
        path,
        workspace.display()
    )
}

pub(super) fn get_path_arg(args: &Value, for_directory: bool) -> Option<String> {
    let path = args.get("path").and_then(|v| v.as_str());
    let alt = if for_directory {
        args.get("directory_path").and_then(|v| v.as_str())
    } else {
        args.get("file_path").and_then(|v| v.as_str())
    };
    path.or(alt).map(String::from)
}

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

// ─── Truncated JSON recovery ─────────────────────────────────────────────────

pub(super) fn parse_truncated_json_for_file_tools(arguments: &str) -> Option<Value> {
    if arguments.is_empty() {
        return None;
    }

    let mut result = serde_json::Map::new();

    if arguments.contains("\"append\":true") {
        result.insert("append".to_string(), Value::Bool(true));
    } else if arguments.contains("\"append\":false") {
        result.insert("append".to_string(), Value::Bool(false));
    }

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

    let content_complete_re =
        regex::Regex::new(r#""content"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
    if let Some(caps) = content_complete_re.captures(arguments) {
        result.insert(
            "content".to_string(),
            Value::String(unescape_json_string(caps.get(1)?.as_str())),
        );
    } else {
        let content_trunc_re = regex::Regex::new(r#""content"\s*:\s*"(.*)$"#).ok()?;
        if let Some(caps) = content_trunc_re.captures(arguments) {
            let mut raw = caps.get(1)?.as_str().to_string();
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

pub(super) fn unescape_json_string(s: &str) -> String {
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
