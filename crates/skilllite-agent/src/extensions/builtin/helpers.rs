//! Shared helpers for the builtin tools module.
//!
//! Security validation, path resolution, directory listing, and truncated JSON recovery.

use crate::error::bail;
use crate::Result;
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

    // JSON "key": "value" 格式（一行可能有多处）
    for k in SENSITIVE_KEYS {
        let pat = format!(r#""{}"\s*:\s*"[^"]*""#, k);
        if let Ok(re) = regex::Regex::new(&pat) {
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

/// Canonicalize an existing root, or lexically normalize when the root is missing.
fn containing_root(root: &Path) -> PathBuf {
    if let Ok(canon) = root.canonicalize() {
        return canon;
    }
    let abs = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(root))
            .unwrap_or_else(|_| root.to_path_buf())
    };
    normalize_path(&abs)
}

/// Reject paths whose final (or nearest existing ancestor) target resolves outside `root_canon`
/// via symlink follow. Lexical `..` escapes are already rejected by the caller.
fn reject_symlink_escape(normalized: &Path, root_canon: &Path, display_path: &str) -> Result<()> {
    let existing = if normalized.exists() {
        Some(normalized.to_path_buf())
    } else {
        let mut ancestor = normalized.parent().map(|p| p.to_path_buf());
        while let Some(ref a) = ancestor {
            if a.exists() {
                break;
            }
            ancestor = a.parent().map(|p| p.to_path_buf());
        }
        ancestor.filter(|a| a.exists())
    };

    let Some(existing) = existing else {
        return Ok(());
    };

    let canon = existing.canonicalize().map_err(|e| {
        crate::Error::validation(format!("Failed to resolve path {}: {}", display_path, e))
    })?;
    if !canon.starts_with(root_canon) {
        bail!(
            "Path escapes workspace via symlink: {} (workspace: {})",
            display_path,
            root_canon.display()
        );
    }
    Ok(())
}

fn resolve_under_root(path: &str, root: &Path) -> Result<PathBuf> {
    let root_canon = containing_root(root);
    let input = Path::new(path);
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        root_canon.join(input)
    };
    let normalized = normalize_path(&resolved);
    if !normalized.starts_with(&root_canon) {
        bail!(
            "Path escapes workspace: {} (workspace: {})",
            path,
            root_canon.display()
        );
    }
    reject_symlink_escape(&normalized, &root_canon, path)?;
    Ok(normalized)
}

pub(super) fn resolve_within_workspace(path: &str, workspace: &Path) -> Result<PathBuf> {
    match resolve_under_root(path, workspace) {
        Ok(normalized) => Ok(normalized),
        Err(err) => {
            // Preserve the write_output hint when the lexically-normalized path lands in output/.
            let input = Path::new(path);
            let resolved = if input.is_absolute() {
                input.to_path_buf()
            } else {
                workspace.join(input)
            };
            let normalized = normalize_path(&resolved);
            let is_output_path =
                types::get_output_dir().is_some_and(|od| normalized.starts_with(Path::new(&od)));
            if is_output_path {
                bail!(
                    "Path escapes workspace: {} (workspace: {}). \
                     Hint: this path is in the output directory — use **write_output** \
                     (with file_path relative to the output dir) instead of write_file.",
                    path,
                    workspace.display()
                );
            }
            Err(err)
        }
    }
}

pub(super) fn resolve_within_workspace_or_output(path: &str, workspace: &Path) -> Result<PathBuf> {
    if let Ok(resolved) = resolve_within_workspace(path, workspace) {
        return Ok(resolved);
    }

    if let Some(output_dir) = types::get_output_dir() {
        let output_root = PathBuf::from(&output_dir);
        if let Ok(resolved) = resolve_under_root(path, &output_root) {
            return Ok(resolved);
        }
    }

    bail!(
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

    let content_complete_re = regex::Regex::new(r#""content"\s*:\s*"((?:[^"\\]|\\.)*)""#).ok()?;
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

#[cfg(test)]
mod path_containment_tests {
    use super::{normalize_path, resolve_under_root, resolve_within_workspace};
    use std::path::{Path, PathBuf};

    fn temp_workspace(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "skilllite_ws_contain_{label}_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create workspace");
        dir
    }

    #[test]
    fn resolve_within_workspace_allows_normal_relative_file() {
        let ws = temp_workspace("ok");
        std::fs::write(ws.join("note.txt"), "hi").unwrap();
        let resolved = resolve_within_workspace("note.txt", &ws).unwrap();
        assert_eq!(resolved, containing_expected(&ws, "note.txt"));
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resolve_within_workspace_rejects_parent_escape() {
        let ws = temp_workspace("dotdot");
        let err = resolve_within_workspace("../outside.txt", &ws).unwrap_err();
        assert!(
            err.to_string().contains("Path escapes workspace"),
            "err={err}"
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resolve_under_root_rejects_symlink_pointing_outside() {
        let ws = temp_workspace("symlink");
        let outside = std::env::temp_dir().join(format!(
            "skilllite_ws_outside_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&outside, "SECRET").unwrap();
        let link = ws.join("leak.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).expect("symlink");
        #[cfg(not(unix))]
        {
            let _ = std::fs::remove_file(&outside);
            let _ = std::fs::remove_dir_all(&ws);
            return;
        }

        let err = resolve_under_root("leak.txt", &ws).unwrap_err();
        assert!(
            err.to_string().contains("symlink"),
            "expected symlink escape error, got: {err}"
        );

        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resolve_under_root_rejects_write_through_symlink_dir() {
        let ws = temp_workspace("symlink_dir");
        let outside_dir = std::env::temp_dir().join(format!(
            "skilllite_ws_outdir_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&outside_dir).unwrap();
        let link_dir = ws.join("out");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_dir, &link_dir).expect("symlink dir");
        #[cfg(not(unix))]
        {
            let _ = std::fs::remove_dir_all(&outside_dir);
            let _ = std::fs::remove_dir_all(&ws);
            return;
        }

        let err = resolve_under_root("out/pwn.txt", &ws).unwrap_err();
        assert!(
            err.to_string().contains("symlink"),
            "expected symlink escape error, got: {err}"
        );

        let _ = std::fs::remove_dir_all(&outside_dir);
        let _ = std::fs::remove_dir_all(&ws);
    }

    fn containing_expected(ws: &Path, rel: &str) -> PathBuf {
        let root = ws.canonicalize().unwrap();
        normalize_path(&root.join(rel))
    }
}
