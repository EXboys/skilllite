//! 卸载信息与退出清理。

#[tauri::command]
pub fn assistant_uninstall_info(
    app: tauri::AppHandle,
) -> Result<crate::uninstall::UninstallInfo, String> {
    crate::uninstall::collect_uninstall_info(&app)
}

#[tauri::command]
pub fn assistant_reveal_install_location() -> Result<(), String> {
    crate::uninstall::reveal_install_location()
}

/// `remove_user_data`: 删除本应用数据目录与 SkillLite `chat/`（会话、transcript 等）。`remove_app_bundle`: macOS 正式包退出后删 .app；Windows 打开「应用和功能」后退出。
#[tauri::command]
pub fn assistant_quit_uninstall(
    app: tauri::AppHandle,
    remove_user_data: bool,
    remove_app_bundle: bool,
) -> Result<(), String> {
    crate::uninstall::quit_uninstall(app, remove_user_data, remove_app_bundle)
}
