//! 工作区根目录解析、子进程 .env、聊天根目录与路径校验（含 Windows）。

use std::path::{Path, PathBuf};
use tauri::Manager;

/// Find project root (dir containing .skills or skills) by walking up from start path.
pub(crate) fn find_project_root(start: &str) -> PathBuf {
    let mut dir = Path::new(start)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(start).to_path_buf());
    for _ in 0..10 {
        if dir.join(".skills").is_dir() || dir.join("skills").is_dir() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    Path::new(start)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(start).to_path_buf())
}

/// Load .env from workspace and parents for subprocess env.
pub(crate) fn load_dotenv_for_child(workspace: &str) -> Vec<(String, String)> {
    skilllite_core::config::parse_dotenv_walking_up(Path::new(workspace), 5)
}

pub(crate) fn skilllite_chat_root() -> PathBuf {
    skilllite_core::paths::chat_root()
}

/// memory/ 与 output/ 下相对路径：禁止绝对路径、`..`、盘符等。
pub(crate) fn validate_chat_subdir_relative(relative_path: &str) -> Result<(), String> {
    if relative_path.is_empty() {
        return Err("Invalid path".to_string());
    }
    if relative_path.contains("..") {
        return Err("Invalid path".to_string());
    }
    let p = Path::new(relative_path);
    if p.is_absolute() {
        return Err("Invalid path".to_string());
    }
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => return Err("Invalid path".to_string()),
            #[cfg(windows)]
            std::path::Component::Prefix(_) => return Err("Invalid path".to_string()),
            _ => {}
        }
    }
    Ok(())
}

/// transcripts 目录下单个日志文件名（无子路径）。
pub(crate) fn validate_transcript_log_filename(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Invalid filename".to_string());
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("Invalid filename".to_string());
    }
    #[cfg(windows)]
    if name.contains(':') {
        return Err("Invalid filename".to_string());
    }
    Ok(())
}

/// Resolve skilllite binary for subprocess (used from `lib` / life_pulse).
pub fn resolve_skilllite_path_app(app: &tauri::AppHandle) -> PathBuf {
    let exe_name = if cfg!(target_os = "windows") {
        "skilllite.exe"
    } else {
        "skilllite"
    };

    #[cfg(debug_assertions)]
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            eprintln!(
                "[skilllite-bridge] using ~/.skilllite/bin binary: {}",
                dev_bin.display()
            );
            return dev_bin;
        }
    }

    if let Ok(res_dir) = app.path().resource_dir() {
        let bundled = res_dir.join(exe_name);
        if bundled.exists() {
            eprintln!(
                "[skilllite-bridge] using bundled binary: {}",
                bundled.display()
            );
            return bundled;
        }
    }

    #[cfg(not(debug_assertions))]
    if let Some(home) = dirs::home_dir() {
        let dev_bin = home.join(".skilllite").join("bin").join(exe_name);
        if dev_bin.exists() {
            eprintln!(
                "[skilllite-bridge] using ~/.skilllite/bin binary: {}",
                dev_bin.display()
            );
            return dev_bin;
        }
    }

    eprintln!(
        "[skilllite-bridge] falling back to PATH lookup for '{}'",
        exe_name
    );
    PathBuf::from(exe_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_chat_subdir_rejects_dotdot_and_absolute() {
        assert!(validate_chat_subdir_relative("a/b").is_ok());
        assert!(validate_chat_subdir_relative("../x").is_err());
        assert!(validate_chat_subdir_relative("foo/../bar").is_err());
        assert!(validate_chat_subdir_relative("/etc/passwd").is_err());
        #[cfg(windows)]
        assert!(validate_chat_subdir_relative(r"\\server\share\x").is_err());
    }

    #[test]
    fn validate_transcript_filename_rejects_path_sep() {
        assert!(validate_transcript_log_filename("default-2026-01-01.jsonl").is_ok());
        assert!(validate_transcript_log_filename("x/y").is_err());
        #[cfg(windows)]
        assert!(validate_transcript_log_filename("C:evil").is_err());
    }
}
