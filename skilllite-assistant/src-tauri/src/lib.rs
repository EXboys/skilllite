#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod gateway_manager;
mod life_pulse;
mod skilllite_bridge;
mod uninstall;
mod windows_spawn;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, RunEvent, WindowEvent,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::chat::skilllite_chat_stream,
            commands::chat::skilllite_stop,
            commands::files_and_dirs::skilllite_load_recent,
            commands::files_and_dirs::skilllite_load_transcript,
            commands::files_and_dirs::skilllite_followup_suggestions,
            commands::files_and_dirs::skilllite_clear_transcript,
            commands::files_and_dirs::skilllite_read_memory_file,
            commands::files_and_dirs::skilllite_read_log_file,
            commands::files_and_dirs::skilllite_read_output_file,
            commands::files_and_dirs::skilllite_read_output_file_base64,
            commands::files_and_dirs::skilllite_read_local_image_b64,
            commands::files_and_dirs::skilllite_open_directory,
            commands::files_and_dirs::skilllite_reveal_in_file_manager,
            commands::files_and_dirs::skilllite_open_skill_directory,
            commands::chat::skilllite_confirm,
            commands::chat::skilllite_clarify,
            commands::skills_workspace::skilllite_list_skills,
            commands::skills_workspace::skilllite_repair_skills,
            commands::skills_workspace::skilllite_add_skill,
            commands::skills_workspace::skilllite_remove_skills,
            commands::skills_workspace::skilllite_default_workspace,
            commands::skills_workspace::skilllite_init_workspace,
            commands::desktop_health::skilllite_probe_ollama,
            commands::desktop_health::skilllite_git_status,
            commands::desktop_health::skilllite_runtime_status,
            commands::desktop_health::skilllite_provision_runtimes,
            commands::desktop_health::skilllite_health_check,
            commands::desktop_health::skilllite_read_schedule,
            commands::desktop_health::skilllite_write_schedule,
            commands::sessions::skilllite_list_sessions,
            commands::sessions::skilllite_create_session,
            commands::sessions::skilllite_rename_session,
            commands::sessions::skilllite_delete_session,
            commands::sessions::skilllite_load_memory_summaries,
            commands::workspace_editor::skilllite_write_workspace_file,
            commands::workspace_editor::skilllite_read_workspace_file,
            commands::workspace_editor::skilllite_resolve_workspace_file_path,
            commands::workspace_editor::skilllite_list_workspace_entries,
            commands::evolution::skilllite_load_evolution_status,
            commands::evolution::skilllite_list_evolution_pending,
            commands::evolution::skilllite_read_pending_skill_md,
            commands::evolution::skilllite_confirm_pending_skill,
            commands::evolution::skilllite_reject_pending_skill,
            commands::evolution::skilllite_authorize_capability_evolution,
            commands::evolution::skilllite_get_evolution_proposal_status,
            commands::evolution::skilllite_load_evolution_backlog,
            commands::evolution::skilllite_trigger_evolution_run,
            commands::evolution::skilllite_load_evolution_diffs,
            commands::evolution::skilllite_list_prompt_snapshot_txns,
            commands::evolution::skilllite_list_prompt_snapshots_batch,
            commands::evolution::skilllite_read_prompt_version_content,
            commands::evolution::skilllite_write_chat_prompt_file,
            commands::life_pulse::skilllite_life_pulse_status,
            commands::life_pulse::skilllite_life_pulse_toggle,
            commands::life_pulse::skilllite_life_pulse_set_workspace,
            commands::life_pulse::skilllite_life_pulse_set_llm_overrides,
            commands::uninstall::assistant_uninstall_info,
            commands::uninstall::assistant_reveal_install_location,
            commands::uninstall::assistant_quit_uninstall,
            commands::gateway::assistant_gateway_health_probe,
            commands::gateway::assistant_gateway_status,
            commands::gateway::assistant_gateway_start,
            commands::gateway::assistant_gateway_stop
        ])
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(skilllite_bridge::ConfirmationState::default())
        .manage(skilllite_bridge::ClarificationState::default())
        .manage(skilllite_bridge::ChatProcessState::default())
        .manage(gateway_manager::GatewayProcessState::default())
        .manage(life_pulse::LifePulseState::default())
        .setup(|app| {
            // ── Life Pulse: start heartbeat thread ──
            let pulse_state = app.state::<life_pulse::LifePulseState>().inner().clone();
            let skilllite_path = skilllite_bridge::resolve_skilllite_path_app(app.handle());
            if let Err(e) = skilllite_bridge::ensure_skilllite_version(&skilllite_path) {
                eprintln!("[skilllite-bridge] {}", e);
            }
            life_pulse::start(pulse_state, skilllite_path, app.handle().clone());

            // Tray icon with menu（中文；失败时通知前端 Toast）
            let show_i = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let app_handle = app.handle().clone();
            match app.default_window_icon() {
                Some(icon) => {
                    if let Err(e) = TrayIconBuilder::new()
                        .icon(icon.clone())
                        .menu(&menu)
                        .show_menu_on_left_click(false)
                        .tooltip("SkillLite 技能助手")
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
                                if let Some(ps) = app.try_state::<life_pulse::LifePulseState>() {
                                    life_pulse::stop(&ps);
                                }
                                app.exit(0);
                            }
                            _ => {}
                        })
                        .build(app)
                    {
                        eprintln!("[skilllite-assistant] tray build failed: {}", e);
                        let _ = app_handle.emit(
                            "skilllite-chrome-bootstrap",
                            serde_json::json!({
                                "kind": "tray",
                                "severity": "error",
                                "message": format!("系统托盘不可用：{}", e)
                            }),
                        );
                    }
                }
                None => {
                    eprintln!("[skilllite-assistant] no default window icon; tray skipped");
                    let _ = app_handle.emit(
                        "skilllite-chrome-bootstrap",
                        serde_json::json!({
                            "kind": "tray",
                            "severity": "info",
                            "message": "未找到应用图标，已跳过系统托盘"
                        }),
                    );
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });
    let run_result = builder.build(tauri::generate_context!()).map(|app| {
        app.run(|app_handle, event| {
            if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                if let Some(ps) = app_handle.try_state::<life_pulse::LifePulseState>() {
                    life_pulse::stop(&ps);
                }
                if let Some(gateway_state) =
                    app_handle.try_state::<gateway_manager::GatewayProcessState>()
                {
                    gateway_manager::cleanup_gateway_process(&gateway_state);
                }
            }
        })
    });
    if let Err(e) = run_result {
        eprintln!("Error running SkillLite Assistant: {}", e);
        std::process::exit(1);
    }
}
