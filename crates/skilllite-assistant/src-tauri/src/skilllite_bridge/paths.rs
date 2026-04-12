//! 工作区根目录解析、子进程 .env、聊天根目录与路径校验（含 Windows）。

use std::path::{Path, PathBuf};
use tauri::Manager;

fn has_known_skill_root(dir: &Path) -> bool {
    skilllite_core::skill::discovery::SKILL_SEARCH_DIRS
        .iter()
        .copied()
        .filter(|entry| *entry != ".")
        .any(|entry| dir.join(entry).is_dir())
}

/// Prefer `Documents/SkillLite` over `"."` so GUI apps (cwd often `/`) do not resolve to `/`.
fn inferred_default_workspace_root() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("SkillLite")
}

/// Find project root (dir containing any supported skill root) by walking up from start path.
pub(crate) fn find_project_root(start: &str) -> PathBuf {
    let t = start.trim();
    let start_path = if t.is_empty() || t == "." {
        inferred_default_workspace_root()
    } else {
        PathBuf::from(t)
    };
    let mut dir = start_path
        .canonicalize()
        .unwrap_or_else(|_| start_path.clone());
    let original = dir.clone();
    let mut best_match = None;
    for _ in 0..10 {
        if has_known_skill_root(&dir) {
            best_match = Some(dir.clone());
        }
        if !dir.pop() {
            break;
        }
    }
    best_match.unwrap_or(original)
}

/// Load .env from workspace and parents for subprocess env.
/// Treats `""` / `"."` like [`find_project_root`] so GUI cwd (`/` or `System32`) does not break lookup.
pub(crate) fn load_dotenv_for_child(workspace: &str) -> Vec<(String, String)> {
    let t = workspace.trim();
    let base = if t.is_empty() || t == "." {
        inferred_default_workspace_root()
    } else {
        PathBuf::from(t)
    };
    skilllite_core::config::parse_dotenv_walking_up(&base, 5)
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

/// Default workspace for first-run / GUI: `Documents/SkillLite` (created if missing).
/// Avoids `"."` which resolves against the app process cwd (often `/` on macOS) and causes
/// permission errors on `/skills` and similar.
pub fn default_writable_workspace_dir() -> Result<String, String> {
    let base = dirs::document_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法解析用户文档或主目录".to_string())?;
    let dir = base.join("SkillLite");
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建默认工作区: {}", e))?;
    let abs = std::fs::canonicalize(&dir).unwrap_or(dir);
    Ok(abs.to_string_lossy().to_string())
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

    #[test]
    fn find_project_root_accepts_nested_skill_roots() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!(
            "skilllite_assistant_paths_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let nested = tmp.join(".agents").join("skills");
        std::fs::create_dir_all(&nested).expect("nested skills");
        let child = nested.join("demo-skill");
        std::fs::create_dir_all(&child).expect("child skill");

        let resolved = find_project_root(child.to_string_lossy().as_ref());
        assert_eq!(
            resolved.canonicalize().expect("resolved canonical"),
            tmp.canonicalize().expect("tmp canonical")
        );
        let _ = std::fs::remove_dir_all(&resolved);
    }
}
