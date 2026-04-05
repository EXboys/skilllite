#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod life_pulse;
mod skilllite_bridge;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};

#[tauri::command]
async fn skilllite_chat_stream(
    window: tauri::Window,
    message: String,
    workspace: Option<String>,
    session_key: Option<String>,
    config: Option<skilllite_bridge::ChatConfigOverrides>,
    images: Option<Vec<skilllite_bridge::ChatImageAttachment>>,
    conf_state: tauri::State<'_, skilllite_bridge::ConfirmationState>,
    clar_state: tauri::State<'_, skilllite_bridge::ClarificationState>,
    process_state: tauri::State<'_, skilllite_bridge::ChatProcessState>,
) -> Result<(), String> {
    let conf = (*conf_state).clone();
    let clar = (*clar_state).clone();
    let proc = (*process_state).clone();
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::chat_stream(
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
fn skilllite_stop(
    process_state: tauri::State<'_, skilllite_bridge::ChatProcessState>,
    conf_state: tauri::State<'_, skilllite_bridge::ConfirmationState>,
    clar_state: tauri::State<'_, skilllite_bridge::ClarificationState>,
) -> Result<(), String> {
    skilllite_bridge::stop_chat(&process_state, &conf_state, &clar_state)
}

#[tauri::command]
async fn skilllite_load_recent(workspace: Option<String>) -> skilllite_bridge::RecentData {
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::load_recent(workspace))
        .await
        .unwrap_or_else(|_| skilllite_bridge::RecentData {
            memory_files: vec![],
            output_files: vec![],
            log_files: vec![],
            plan: None,
        })
}

#[tauri::command]
async fn skilllite_load_transcript(
    session_key: Option<String>,
) -> Vec<skilllite_bridge::TranscriptMessage> {
    let key = session_key.unwrap_or_else(|| "default".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::load_transcript(&key))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_clear_transcript(
    app: tauri::AppHandle,
    session_key: Option<String>,
    workspace: Option<String>,
) -> Result<(), String> {
    let key = session_key.unwrap_or_else(|| "default".to_string());
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::clear_transcript(&key, &ws, &path)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(std::convert::identity)
}

#[tauri::command]
async fn skilllite_read_memory_file(relative_path: String) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_memory_file(&path))
        .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_read_log_file(filename: String) -> Result<String, String> {
    let name = filename.clone();
    match tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_log_file(&name))
        .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_read_output_file(
    relative_path: String,
    workspace: Option<String>,
) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::read_output_file(&path, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_read_output_file_base64(
    relative_path: String,
    workspace: Option<String>,
) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::read_output_file_base64(&path, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_open_directory(
    module: String,
    workspace: Option<String>,
) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::open_directory(&module, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_reveal_in_file_manager(path: String) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::reveal_in_file_manager(&path)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn skilllite_open_skill_directory(
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::open_skill_directory(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
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

#[tauri::command]
fn skilllite_clarify(
    app: tauri::AppHandle,
    action: String,
    hint: Option<String>,
) -> Result<(), String> {
    let state = app.state::<skilllite_bridge::ClarificationState>();
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "ClarificationState lock poisoned")?;
    if let Some(tx) = guard.take() {
        let _ = tx.send(skilllite_bridge::ClarifyResponse { action, hint });
    }
    Ok(())
}

#[tauri::command]
async fn skilllite_list_skills(workspace: Option<String>) -> Vec<String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::list_skill_names(&ws))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_repair_skills(
    app: tauri::AppHandle,
    workspace: Option<String>,
    skill_names: Vec<String>,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::repair_skills(&ws, &skill_names, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_add_skill(
    app: tauri::AppHandle,
    workspace: Option<String>,
    source: String,
    force: bool,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::add_skill(&ws, &source, force, &path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_remove_skills(
    workspace: Option<String>,
    skill_names: Vec<String>,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::remove_skills(&ws, &skill_names))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_init_workspace(app: tauri::AppHandle, dir: String) -> Result<(), String> {
    let path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::init_workspace(&dir, &path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_list_sessions() -> Vec<skilllite_bridge::SessionInfo> {
    tauri::async_runtime::spawn_blocking(skilllite_bridge::list_sessions)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_create_session(
    display_name: String,
) -> Result<skilllite_bridge::SessionInfo, String> {
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::create_session(&display_name))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_rename_session(
    session_key: String,
    new_name: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::rename_session(&session_key, &new_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_delete_session(session_key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::delete_session(&session_key))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_load_memory_summaries() -> Vec<skilllite_bridge::MemoryEntry> {
    tauri::async_runtime::spawn_blocking(skilllite_bridge::load_memory_summaries)
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_write_workspace_file(
    workspace: String,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    let ws = workspace.trim().to_string();
    let rel = relative_path;
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::write_workspace_text_file(&ws, &rel, &content)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_read_workspace_file(
    workspace: String,
    relative_path: String,
) -> Result<String, String> {
    let ws = workspace.trim().to_string();
    let rel = relative_path;
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::read_workspace_text_file(&ws, &rel)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_list_workspace_entries(
    workspace: String,
) -> Result<Vec<skilllite_bridge::WorkspaceListEntry>, String> {
    let ws = workspace.trim().to_string();
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::list_workspace_entries(&ws))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_load_evolution_status(
    workspace: Option<String>,
    config: Option<skilllite_bridge::ChatConfigOverrides>,
) -> skilllite_bridge::EvolutionStatusPayload {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::load_evolution_status(&ws, config)
    })
        .await
        .unwrap_or_else(|e| skilllite_bridge::EvolutionStatusPayload {
            mode_key: "error".into(),
            mode_label: format!("任务失败: {}", e),
            interval_secs: 600,
            decision_threshold: 10,
            weighted_signal_sum: 0,
            weighted_trigger_min: 3,
            signal_window: 10,
            evo_profile_key: "default".into(),
            evo_cooldown_hours: 1.0,
            unprocessed_decisions: 0,
            last_run_ts: None,
            judgement_label: None,
            judgement_reason: None,
            recent_events: vec![],
            pending_skill_count: 0,
            db_error: Some(e.to_string()),
        })
}

#[tauri::command]
async fn skilllite_list_evolution_pending(
    workspace: Option<String>,
) -> Vec<skilllite_bridge::PendingSkillDto> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::list_evolution_pending_skills(&ws))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_read_pending_skill_md(
    workspace: Option<String>,
    skill_name: String,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::read_evolution_pending_skill_md(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_confirm_pending_skill(
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::evolution_confirm_pending_skill(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_reject_pending_skill(
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::evolution_reject_pending_skill(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_authorize_capability_evolution(
    app: tauri::AppHandle,
    workspace: Option<String>,
    tool_name: String,
    outcome: String,
    summary: String,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::authorize_capability_evolution(
            &ws,
            &tool_name,
            &outcome,
            &summary,
            &skilllite_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_get_evolution_proposal_status(
    workspace: Option<String>,
    proposal_id: String,
) -> Result<skilllite_bridge::EvolutionProposalStatusDto, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::get_evolution_proposal_status(&ws, &proposal_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_load_evolution_backlog(
    workspace: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<skilllite_bridge::EvolutionBacklogRowDto>, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let capped = limit.unwrap_or(30) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::load_evolution_backlog(&ws, capped)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_trigger_evolution_run(
    app: tauri::AppHandle,
    workspace: Option<String>,
    proposal_id: Option<String>,
    config: Option<skilllite_bridge::ChatConfigOverrides>,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::trigger_evolution_run(
            &ws,
            proposal_id.as_deref(),
            &skilllite_path,
            config,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_load_evolution_diffs(
    workspace: Option<String>,
) -> Vec<skilllite_bridge::EvolutionFileDiffDto> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::load_evolution_diffs(&ws))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn skilllite_probe_ollama() -> skilllite_bridge::OllamaProbeResult {
    tauri::async_runtime::spawn_blocking(skilllite_bridge::probe_ollama)
        .await
        .unwrap_or_else(|_| skilllite_bridge::OllamaProbeResult {
            available: false,
            models: vec![],
            has_embedding: false,
        })
}

#[tauri::command]
async fn skilllite_runtime_status() -> skilllite_bridge::RuntimeUiSnapshot {
    match tauri::async_runtime::spawn_blocking(skilllite_bridge::probe_runtime_status).await {
        Ok(s) => s,
        Err(_) => skilllite_bridge::RuntimeUiSnapshot {
            python: skilllite_bridge::RuntimeUiLine {
                source: "none".into(),
                label: "加载失败".into(),
                reveal_path: None,
                detail: None,
            },
            node: skilllite_bridge::RuntimeUiLine {
                source: "none".into(),
                label: "加载失败".into(),
                reveal_path: None,
                detail: None,
            },
            cache_root: None,
            cache_root_abs: None,
        },
    }
}

#[tauri::command]
async fn skilllite_provision_runtimes(
    app: tauri::AppHandle,
    python: bool,
    node: bool,
    force: bool,
) -> Result<skilllite_bridge::ProvisionRuntimesResult, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::provision_runtimes_with_emit(&app, python, node, force)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn skilllite_read_schedule(workspace: String) -> Result<String, String> {
    let ws = workspace.trim().to_string();
    if ws.is_empty() {
        return Err("工作区路径无效".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::read_schedule_json(&ws))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_write_schedule(workspace: String, json: String) -> Result<(), String> {
    let ws = workspace.trim().to_string();
    if ws.is_empty() {
        return Err("工作区路径无效".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || skilllite_bridge::write_schedule_json(&ws, &json))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn skilllite_health_check(
    app: tauri::AppHandle,
    workspace: String,
    provider: skilllite_bridge::OnboardingProvider,
    api_key: Option<String>,
) -> skilllite_bridge::OnboardingHealthCheckResult {
    let path = skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        skilllite_bridge::run_onboarding_health_check(
            &path,
            &workspace,
            provider,
            api_key.as_deref(),
        )
    })
    .await
    .unwrap_or_else(|e| skilllite_bridge::OnboardingHealthCheckResult {
        binary: skilllite_bridge::HealthCheckItem {
            ok: false,
            message: format!("健康检查任务执行失败：{}", e),
        },
        provider: skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行 provider 检查".to_string(),
        },
        workspace: skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行工作区检查".to_string(),
        },
        data_dir: skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行数据目录检查".to_string(),
        },
        ok: false,
    })
}

#[tauri::command]
fn skilllite_life_pulse_status(
    state: tauri::State<'_, life_pulse::LifePulseState>,
) -> life_pulse::LifePulseStatus {
    state.status()
}

#[tauri::command]
fn skilllite_life_pulse_toggle(
    state: tauri::State<'_, life_pulse::LifePulseState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
fn skilllite_life_pulse_set_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, life_pulse::LifePulseState>,
    workspace: String,
) -> Result<(), String> {
    state.set_workspace(&workspace);
    if let Err(e) = skilllite_bridge::sync_bundled_skills_from_resources(&app, &workspace) {
        eprintln!("[skilllite-assistant] bundled skills sync failed: {}", e);
    }
    Ok(())
}

#[tauri::command]
fn skilllite_life_pulse_set_llm_overrides(
    state: tauri::State<'_, life_pulse::LifePulseState>,
    config: Option<skilllite_bridge::ChatConfigOverrides>,
) -> Result<(), String> {
    state.set_llm_overrides(config);
    Ok(())
}

/// Read a user-picked image from disk (after native file dialog) for chat attachments.
#[tauri::command]
fn skilllite_read_local_image_b64(path: String) -> Result<skilllite_bridge::ChatImageAttachment, String> {
    use base64::Engine;
    use std::path::Path;

    const MAX_BYTES: u64 = 5 * 1024 * 1024;

    let p = Path::new(&path);
    let meta = std::fs::metadata(p).map_err(|e| format!("Cannot read file: {}", e))?;
    if !meta.is_file() {
        return Err("Path is not a file".to_string());
    }
    if meta.len() > MAX_BYTES {
        return Err("Image exceeds 5MB limit".to_string());
    }

    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let media_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => {
            return Err(format!(
                "Unsupported extension .{} (use png, jpg, webp, or gif)",
                ext
            ));
        }
    };

    let bytes = std::fs::read(p).map_err(|e| format!("Failed to read image: {}", e))?;
    let data_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(skilllite_bridge::ChatImageAttachment {
        media_type: media_type.to_string(),
        data_base64,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let run_result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            skilllite_chat_stream,
            skilllite_stop,
            skilllite_load_recent,
            skilllite_load_transcript,
            skilllite_clear_transcript,
            skilllite_read_memory_file,
            skilllite_read_log_file,
            skilllite_read_output_file,
            skilllite_read_output_file_base64,
            skilllite_read_local_image_b64,
            skilllite_open_directory,
            skilllite_reveal_in_file_manager,
            skilllite_open_skill_directory,
            skilllite_confirm,
            skilllite_clarify,
            skilllite_list_skills,
            skilllite_repair_skills,
            skilllite_add_skill,
            skilllite_remove_skills,
            skilllite_init_workspace,
            skilllite_probe_ollama,
            skilllite_runtime_status,
            skilllite_provision_runtimes,
            skilllite_health_check,
            skilllite_read_schedule,
            skilllite_write_schedule,
            skilllite_list_sessions,
            skilllite_create_session,
            skilllite_rename_session,
            skilllite_delete_session,
            skilllite_load_memory_summaries,
            skilllite_write_workspace_file,
            skilllite_read_workspace_file,
            skilllite_list_workspace_entries,
            skilllite_load_evolution_status,
            skilllite_list_evolution_pending,
            skilllite_read_pending_skill_md,
            skilllite_confirm_pending_skill,
            skilllite_reject_pending_skill,
            skilllite_authorize_capability_evolution,
            skilllite_get_evolution_proposal_status,
            skilllite_load_evolution_backlog,
            skilllite_trigger_evolution_run,
            skilllite_load_evolution_diffs,
            skilllite_life_pulse_status,
            skilllite_life_pulse_toggle,
            skilllite_life_pulse_set_workspace,
            skilllite_life_pulse_set_llm_overrides
        ])
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(skilllite_bridge::ConfirmationState::default())
        .manage(skilllite_bridge::ClarificationState::default())
        .manage(skilllite_bridge::ChatProcessState::default())
        .manage(life_pulse::LifePulseState::default())
        .setup(|app| {
            // ── Life Pulse: start heartbeat thread ──
            let pulse_state = app.state::<life_pulse::LifePulseState>().inner().clone();
            let skilllite_path = skilllite_bridge::resolve_skilllite_path_app(app.handle());
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
        })
        .run(tauri::generate_context!());
    if let Err(e) = run_result {
        eprintln!("Error running SkillLite Assistant: {}", e);
        std::process::exit(1);
    }
}
