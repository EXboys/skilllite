//! Life Pulse 状态与与工作区 / LLM 覆盖同步。

#[tauri::command]
pub fn skilllite_life_pulse_status(
    state: tauri::State<'_, crate::life_pulse::LifePulseState>,
) -> crate::life_pulse::LifePulseStatus {
    state.status()
}

#[tauri::command]
pub fn skilllite_life_pulse_toggle(
    state: tauri::State<'_, crate::life_pulse::LifePulseState>,
    enabled: bool,
) -> Result<(), String> {
    state.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
pub fn skilllite_life_pulse_set_workspace(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::life_pulse::LifePulseState>,
    workspace: String,
) -> Result<(), String> {
    state.set_workspace(&workspace);
    if let Err(e) = crate::skilllite_bridge::sync_bundled_skills_from_resources(&app, &workspace) {
        eprintln!("[skilllite-assistant] bundled skills sync failed: {}", e);
    }
    Ok(())
}

#[tauri::command]
pub fn skilllite_life_pulse_set_llm_overrides(
    state: tauri::State<'_, crate::life_pulse::LifePulseState>,
    config: Option<crate::skilllite_bridge::ChatConfigOverrides>,
) -> Result<(), String> {
    state.set_llm_overrides(config);
    Ok(())
}
