//! 会话与 memory 摘要。

#[tauri::command]
pub async fn skilllite_list_sessions() -> Vec<crate::skilllite_bridge::SessionInfo> {
    tauri::async_runtime::spawn_blocking(crate::skilllite_bridge::list_sessions)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn skilllite_create_session(
    display_name: String,
) -> Result<crate::skilllite_bridge::SessionInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::create_session(&display_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_rename_session(session_key: String, new_name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::rename_session(&session_key, &new_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_delete_session(session_key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::delete_session(&session_key)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_load_memory_summaries() -> Vec<crate::skilllite_bridge::MemoryEntry> {
    tauri::async_runtime::spawn_blocking(crate::skilllite_bridge::load_memory_summaries)
        .await
        .unwrap_or_default()
}
