//! 技能目录与默认工作区初始化。

#[tauri::command]
pub async fn skilllite_list_skills(
    app: tauri::AppHandle,
    workspace: Option<String>,
) -> Vec<crate::skilllite_bridge::DesktopSkillInfo> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::list_skills(&ws, Some(&path))
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn skilllite_repair_skills(
    app: tauri::AppHandle,
    workspace: Option<String>,
    skill_names: Vec<String>,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::repair_skills(&ws, &skill_names, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_add_skill(
    app: tauri::AppHandle,
    workspace: Option<String>,
    source: String,
    force: bool,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::add_skill(&ws, &source, force, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_remove_skills(
    workspace: Option<String>,
    skill_names: Vec<String>,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::remove_skills(&ws, &skill_names)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn skilllite_default_workspace() -> Result<String, String> {
    crate::skilllite_bridge::default_writable_workspace_dir()
}

#[tauri::command]
pub async fn skilllite_init_workspace(app: tauri::AppHandle, dir: String) -> Result<(), String> {
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::init_workspace(&dir, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}
