//! 卸载引导：展示安装位置、删除本机数据、退出后删除应用包（macOS 打包版）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallInfo {
    pub platform: String,
    /// 当前可执行文件所在目录（安装根或 .app/Contents/MacOS）
    pub executable_parent: String,
    /// macOS 下 .app 路径；其他平台为 null
    pub macos_app_bundle_path: Option<String>,
    pub tauri_app_data_dir: String,
    pub skilllite_chat_root: String,
    pub skilllite_data_root: String,
    /// 正式打包且为 macOS 时，可在退出后尝试删除 .app
    pub can_schedule_macos_bundle_removal: bool,
    /// 开发构建不执行删除 .app，避免删掉 target 产物
    pub is_dev_build: bool,
}

fn resolve_macos_app_bundle(exe: &Path) -> Option<PathBuf> {
    let mut p = exe.to_path_buf();
    while p.pop() {
        if p.extension().is_some_and(|e| e == "app") {
            return Some(p);
        }
    }
    None
}

pub fn collect_uninstall_info(app: &tauri::AppHandle) -> Result<UninstallInfo, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let macos_app_bundle_path = resolve_macos_app_bundle(&exe).map(|p| p.to_string_lossy().to_string());

    let tauri_app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    let chat = skilllite_core::paths::chat_root();
    let data_root = skilllite_core::paths::data_root();

    Ok(UninstallInfo {
        platform: std::env::consts::OS.to_string(),
        executable_parent: parent,
        macos_app_bundle_path,
        tauri_app_data_dir,
        skilllite_chat_root: chat.to_string_lossy().to_string(),
        skilllite_data_root: data_root.to_string_lossy().to_string(),
        can_schedule_macos_bundle_removal: cfg!(target_os = "macos") && !cfg!(debug_assertions),
        is_dev_build: cfg!(debug_assertions),
    })
}

/// 在访达 / 资源管理器中定位安装位置（macOS 优先高亮 .app）。
pub fn reveal_install_location() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.canonicalize().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    if let Some(bundle) = resolve_macos_app_bundle(&exe) {
        let b = bundle.canonicalize().map_err(|e| e.to_string())?;
        Command::new("open")
            .args(["-R", b.to_str().ok_or("无效路径")?])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    crate::skilllite_bridge::reveal_in_file_manager(exe.to_str().ok_or("无效路径")?)
}

fn remove_dir_all_logged(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path)
        .map_err(|e| format!("无法删除 {}: {}", path.display(), e))
}

/// 删除 Tauri 应用数据目录与（可选）SkillLite `chat/` 树（会话、transcript 等）。
/// 不删除 `~/.skilllite/bin` 等 CLI 工具链，除非用户另行处理。
pub fn remove_local_user_data(app: &tauri::AppHandle, include_skilllite_chat: bool) -> Result<(), String> {
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    remove_dir_all_logged(&app_data)?;

    if include_skilllite_chat {
        let chat = skilllite_core::paths::chat_root();
        remove_dir_all_logged(&chat)?;
    }
    Ok(())
}

fn shell_single_quoted_path(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\"'\"'"))
}

/// 启动独立子进程，在约 2 秒后删除 .app（当前进程可先退出）。
#[cfg(target_os = "macos")]
fn schedule_macos_bundle_removal(bundle: &Path) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let bundle = bundle.canonicalize().map_err(|e| e.to_string())?;
    if !bundle.exists() {
        return Ok(());
    }
    let quoted = shell_single_quoted_path(&bundle.to_string_lossy());
    let cmd = format!("sleep 2; rm -rf {quoted}");

    Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法启动卸载清理进程: {}", e))?;
    Ok(())
}

/// 打开 Windows「应用和功能」以便卸载本应用。
#[cfg(target_os = "windows")]
fn open_windows_apps_settings() -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:appsfeatures"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// `remove_user_data`: 本应用 WebView 存储、设置持久化等 + 可选 `chat/`。
/// `remove_app_bundle`: macOS 正式包尝试退出后删 .app；Windows 打开系统卸载页；其他平台仅依赖「显示安装位置」。
pub fn quit_uninstall(
    app: tauri::AppHandle,
    remove_user_data: bool,
    remove_app_bundle: bool,
) -> Result<(), String> {
    if remove_user_data {
        remove_local_user_data(&app, true)?;
    }

    if remove_app_bundle {
        #[cfg(target_os = "macos")]
        {
            if !cfg!(debug_assertions) {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                if let Some(bundle) = resolve_macos_app_bundle(&exe) {
                    schedule_macos_bundle_removal(&bundle)?;
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            let _ = open_windows_apps_settings();
        }
    }

    if let Some(ps) = app.try_state::<crate::life_pulse::LifePulseState>() {
        crate::life_pulse::stop(&ps);
    }

    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_macos_bundle_from_exe() {
        let exe = PathBuf::from("/Applications/SkillLite Assistant.app/Contents/MacOS/skilllite-assistant");
        let b = resolve_macos_app_bundle(&exe).expect("bundle");
        assert_eq!(
            b,
            PathBuf::from("/Applications/SkillLite Assistant.app")
        );
    }

    #[test]
    fn shell_quote_escapes_single_quote() {
        let s = shell_single_quoted_path("/tmp/a'b");
        assert!(s.contains("'\"'\"'"));
    }
}
