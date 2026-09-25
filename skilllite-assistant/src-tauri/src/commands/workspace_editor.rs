//! 工作区内文本文件的读写与枚举。

#[tauri::command]
pub async fn skilllite_write_workspace_file(
    workspace: String,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    let ws = workspace.trim().to_string();
    let rel = relative_path;
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::write_workspace_text_file(&ws, &rel, &content)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_read_workspace_file(
    workspace: String,
    relative_path: String,
) -> Result<String, String> {
    let ws = workspace.trim().to_string();
    let rel = relative_path;
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_workspace_text_file(&ws, &rel)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_resolve_workspace_file_path(
    workspace: String,
    relative_path: String,
) -> Result<String, String> {
    let ws = workspace.trim().to_string();
    let rel = relative_path;
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::resolve_workspace_existing_file_path(&ws, &rel)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_list_workspace_entries(
    workspace: String,
) -> Result<crate::skilllite_bridge::WorkspaceListEntriesPayload, String> {
    let ws = workspace.trim().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::list_workspace_entries(&ws)
    })
    .await
    .map_err(|e| e.to_string())?
}
