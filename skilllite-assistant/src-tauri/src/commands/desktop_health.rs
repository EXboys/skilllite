//! Ollama / Git / 运行时、引导体检与 schedule.json。

#[tauri::command]
pub async fn skilllite_probe_ollama() -> crate::skilllite_bridge::OllamaProbeResult {
    tauri::async_runtime::spawn_blocking(crate::skilllite_bridge::probe_ollama)
        .await
        .unwrap_or_else(|_| crate::skilllite_bridge::OllamaProbeResult {
            available: false,
            models: vec![],
            has_embedding: false,
        })
}

#[tauri::command]
pub async fn skilllite_git_status() -> crate::skilllite_bridge::GitUiStatus {
    match tauri::async_runtime::spawn_blocking(crate::skilllite_bridge::probe_git_status).await {
        Ok(s) => s,
        Err(e) => crate::skilllite_bridge::GitUiStatus {
            available: false,
            version_line: None,
            error_detail: Some(format!("task failed: {}", e)),
        },
    }
}

#[tauri::command]
pub async fn skilllite_runtime_status(
    app: tauri::AppHandle,
    workspace: Option<String>,
) -> crate::skilllite_bridge::RuntimeUiSnapshot {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::probe_runtime_status(&ws, &path)
    })
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(_)) | Err(_) => crate::skilllite_bridge::RuntimeUiSnapshot {
            python: crate::skilllite_bridge::RuntimeUiLine {
                source: "none".into(),
                label: "加载失败".into(),
                reveal_path: None,
                detail: None,
            },
            node: crate::skilllite_bridge::RuntimeUiLine {
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
pub async fn skilllite_provision_runtimes(
    app: tauri::AppHandle,
    workspace: Option<String>,
    python: bool,
    node: bool,
    force: bool,
) -> Result<crate::skilllite_bridge::ProvisionRuntimesResult, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::provision_runtimes_with_emit(&app, &ws, &path, python, node, force)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn skilllite_health_check(
    app: tauri::AppHandle,
    input: crate::skilllite_bridge::OnboardingHealthCheckInput,
) -> crate::skilllite_bridge::OnboardingHealthCheckResult {
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    let workspace = input.workspace.trim().to_string();
    let provider = input.provider;
    let api_key = input.api_key;
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::run_onboarding_health_check(
            &path,
            &workspace,
            provider,
            api_key.as_deref(),
        )
    })
    .await
    .unwrap_or_else(|e| crate::skilllite_bridge::OnboardingHealthCheckResult {
        binary: crate::skilllite_bridge::HealthCheckItem {
            ok: false,
            message: format!("健康检查任务执行失败：{}", e),
        },
        provider: crate::skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行 provider 检查".to_string(),
        },
        workspace: crate::skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行工作区检查".to_string(),
        },
        data_dir: crate::skilllite_bridge::HealthCheckItem {
            ok: false,
            message: "未执行数据目录检查".to_string(),
        },
        ok: false,
    })
}

#[tauri::command]
pub async fn skilllite_read_schedule(workspace: String) -> Result<String, String> {
    let ws = workspace.trim().to_string();
    if ws.is_empty() {
        return Err("工作区路径无效".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || crate::skilllite_bridge::read_schedule_json(&ws))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_write_schedule(workspace: String, json: String) -> Result<(), String> {
    let ws = workspace.trim().to_string();
    if ws.is_empty() {
        return Err("工作区路径无效".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::write_schedule_json(&ws, &json)
    })
    .await
    .map_err(|e| e.to_string())?
}
