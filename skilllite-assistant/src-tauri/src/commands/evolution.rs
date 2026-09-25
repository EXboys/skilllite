//! 进化状态、待审核技能、Backlog、触发 run 与 prompt 快照相关 invoke。

#[tauri::command]
pub async fn skilllite_load_evolution_status(
    app: tauri::AppHandle,
    workspace: Option<String>,
    config: Option<crate::skilllite_bridge::ChatConfigOverrides>,
    life_pulse: tauri::State<'_, crate::life_pulse::LifePulseState>,
) -> Result<
    crate::skilllite_bridge::LlmInvokeResult<crate::skilllite_bridge::EvolutionStatusPayload>,
    String,
> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let periodic_anchor_unix = life_pulse.periodic_anchor_unix();
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    let out = match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::load_evolution_status(
            &ws,
            config,
            periodic_anchor_unix,
            &skilllite_path,
        )
    })
    .await
    {
        Ok(Ok(payload)) => crate::skilllite_bridge::LlmInvokeResult::ok(payload),
        Ok(Err(err)) => crate::skilllite_bridge::LlmInvokeResult::err(
            crate::skilllite_bridge::classify_llm_routing_error_message(&err),
        ),
        Err(err) => crate::skilllite_bridge::LlmInvokeResult::err(
            crate::skilllite_bridge::classify_llm_routing_error_message(&err.to_string()),
        ),
    };
    Ok(out)
}

#[tauri::command]
pub async fn skilllite_list_evolution_pending(
    app: tauri::AppHandle,
    workspace: Option<String>,
) -> Vec<crate::skilllite_bridge::PendingSkillDto> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::list_evolution_pending_skills(&ws, &skilllite_path)
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn skilllite_read_pending_skill_md(
    workspace: Option<String>,
    skill_name: String,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_evolution_pending_skill_md(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_confirm_pending_skill(
    app: tauri::AppHandle,
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::evolution_confirm_pending_skill(
            &ws,
            &skill_name,
            &skilllite_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_reject_pending_skill(
    app: tauri::AppHandle,
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::evolution_reject_pending_skill(
            &ws,
            &skill_name,
            &skilllite_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_authorize_capability_evolution(
    app: tauri::AppHandle,
    workspace: Option<String>,
    tool_name: String,
    outcome: String,
    summary: String,
) -> Result<String, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::authorize_capability_evolution(
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
pub async fn skilllite_get_evolution_proposal_status(
    app: tauri::AppHandle,
    workspace: Option<String>,
    proposal_id: String,
) -> Result<crate::skilllite_bridge::EvolutionProposalStatusDto, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::get_evolution_proposal_status(
            &ws,
            &proposal_id,
            &skilllite_path,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_load_evolution_backlog(
    app: tauri::AppHandle,
    workspace: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<crate::skilllite_bridge::EvolutionBacklogRowDto>, String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let capped = limit.unwrap_or(30) as usize;
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::load_evolution_backlog(&ws, capped, &skilllite_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn skilllite_trigger_evolution_run(
    app: tauri::AppHandle,
    workspace: Option<String>,
    proposal_id: Option<String>,
    config: Option<crate::skilllite_bridge::ChatConfigOverrides>,
) -> crate::skilllite_bridge::LlmInvokeResult<String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let skilllite_path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::trigger_evolution_run(
            &ws,
            proposal_id.as_deref(),
            &skilllite_path,
            config,
        )
    })
    .await
    {
        Ok(Ok(out)) => crate::skilllite_bridge::LlmInvokeResult::ok(out),
        Ok(Err(err)) => crate::skilllite_bridge::LlmInvokeResult::err(
            crate::skilllite_bridge::classify_llm_routing_error_message(&err),
        ),
        Err(err) => crate::skilllite_bridge::LlmInvokeResult::err(
            crate::skilllite_bridge::classify_llm_routing_error_message(&err.to_string()),
        ),
    }
}

#[tauri::command]
pub async fn skilllite_load_evolution_diffs(
    workspace: Option<String>,
) -> Vec<crate::skilllite_bridge::EvolutionFileDiffDto> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || crate::skilllite_bridge::load_evolution_diffs(&ws))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn skilllite_list_prompt_snapshot_txns(
    filename: String,
) -> Result<Vec<crate::skilllite_bridge::EvolutionSnapshotTxnDto>, String> {
    let f = filename;
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::list_prompt_snapshot_txns(&f)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_list_prompt_snapshots_batch(
    filenames: Vec<String>,
) -> Result<
    std::collections::HashMap<String, Vec<crate::skilllite_bridge::EvolutionSnapshotTxnDto>>,
    String,
> {
    let names = filenames;
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::list_prompt_snapshots_batch(&names)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_read_prompt_version_content(
    filename: String,
    version_ref: String,
) -> Result<String, String> {
    let f = filename;
    let v = version_ref;
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_prompt_version_content(&f, &v)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_write_chat_prompt_file(
    filename: String,
    content: String,
) -> Result<(), String> {
    let f = filename;
    let c = content;
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::write_chat_prompt_text_file(&f, &c)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}
