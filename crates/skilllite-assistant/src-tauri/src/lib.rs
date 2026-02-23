#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod skilllite_bridge;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

#[tauri::command]
async fn skilllite_chat_stream(
    window: tauri::Window,
    message: String,
    workspace: Option<String>,
    config: Option<skilllite_bridge::ChatConfigOverrides>,
    conf_state: tauri::State<'_, skilllite_bridge::ConfirmationState>,
    process_state: tauri::State<'_, skilllite_bridge::ChatProcessState>,
) -> Result<(), String> {
    let conf = (*conf_state).clone();
    let proc = (*process_state).clone();
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::chat_stream(window, message, workspace, config, conf, proc)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn skilllite_stop(process_state: tauri::State<'_, skilllite_bridge::ChatProcessState>) -> Result<(), String> {
    skilllite_bridge::stop_chat(&process_state)
}

#[tauri::command]
async fn skilllite_load_recent() -> skilllite_bridge::RecentData {
    tauri::async_runtime::spawn_blocking(skilllite_bridge::load_recent)
        .await
        .unwrap_or_else(|_| skilllite_bridge::RecentData {
            memory_files: vec![],
            output_files: vec![],
            plan: None,
        })
}

#[tauri::command]
async fn skilllite_load_transcript(session_key: Option<String>) -> Vec<skilllite_bridge::TranscriptMessage> {
    let key = session_key.unwrap_or_else(|| "default".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::load_transcript(&key))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_read_memory_file(relative_path: String) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_memory_file(&path)).await {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_read_output_file(relative_path: String) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_output_file(&path)).await {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_read_output_file_base64(relative_path: String) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_output_file_base64(&path)).await {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_open_directory(module: String) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::open_directory(&module)).await {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn skilllite_confirm(app: tauri::AppHandle, approved: bool) -> Result<(), String> {
    let state = app.state::<skilllite_bridge::ConfirmationState>();
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "ConfirmationState lock poisoned")?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(approved);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            skilllite_chat_stream,
            skilllite_stop,
            skilllite_load_recent,
            skilllite_load_transcript,
            skilllite_read_memory_file,
            skilllite_read_output_file,
            skilllite_read_output_file_base64,
            skilllite_open_directory,
            skilllite_confirm
        ])
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(skilllite_bridge::ConfirmationState::default())
        .manage(skilllite_bridge::ChatProcessState::default())
        .setup(|app| {
            // Tray icon with menu
            let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("SkillLite Assistant")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = tray.app_handle().get_webview_window("main").map(|w| {
                            let _ = w.show();
                            let _ = w.set_focus();
                        });
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running SkillLite Assistant");
}
