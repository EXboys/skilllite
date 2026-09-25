//! 聊天流、停止、确认与澄清通道。

use tauri::Manager;

#[tauri::command]
// Tauri command wiring requires multiple explicit payload/state parameters here.
#[allow(clippy::too_many_arguments)]
pub async fn skilllite_chat_stream(
    window: tauri::Window,
    message: String,
    workspace: Option<String>,
    session_key: Option<String>,
    config: Option<crate::skilllite_bridge::ChatConfigOverrides>,
    images: Option<Vec<crate::skilllite_bridge::ChatImageAttachment>>,
    conf_state: tauri::State<'_, crate::skilllite_bridge::ConfirmationState>,
    clar_state: tauri::State<'_, crate::skilllite_bridge::ClarificationState>,
    process_state: tauri::State<'_, crate::skilllite_bridge::ChatProcessState>,
) -> Result<(), String> {
    let conf = (*conf_state).clone();
    let clar = (*clar_state).clone();
    let proc = (*process_state).clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::chat_stream(
            window,
            message,
            workspace,
            config,
            session_key,
            images,
            conf,
            clar,
            proc,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn skilllite_stop(
    process_state: tauri::State<'_, crate::skilllite_bridge::ChatProcessState>,
    conf_state: tauri::State<'_, crate::skilllite_bridge::ConfirmationState>,
    clar_state: tauri::State<'_, crate::skilllite_bridge::ClarificationState>,
) -> Result<(), String> {
    crate::skilllite_bridge::stop_chat(&process_state, &conf_state, &clar_state)
}

#[tauri::command]
pub fn skilllite_confirm(app: tauri::AppHandle, approved: bool) -> Result<(), String> {
    let state = app.state::<crate::skilllite_bridge::ConfirmationState>();
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "ConfirmationState lock poisoned")?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(approved);
    }
    Ok(())
}

#[tauri::command]
pub fn skilllite_clarify(
    app: tauri::AppHandle,
    action: String,
    hint: Option<String>,
) -> Result<(), String> {
    let state = app.state::<crate::skilllite_bridge::ClarificationState>();
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "ClarificationState lock poisoned")?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(crate::skilllite_bridge::ClarifyResponse { action, hint });
    }
    Ok(())
}
