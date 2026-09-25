//! 工作区浏览：最近文件、transcript、读写 memory/log/output、打开目录、聊天附件图片。

#[tauri::command]
pub async fn skilllite_load_recent(
    workspace: Option<String>,
) -> crate::skilllite_bridge::RecentData {
    tauri::async_runtime::spawn_blocking(move || crate::skilllite_bridge::load_recent(workspace))
        .await
        .unwrap_or_else(|_| crate::skilllite_bridge::RecentData {
            memory_files: vec![],
            output_files: vec![],
            log_files: vec![],
            plan: None,
        })
}

#[tauri::command]
pub async fn skilllite_load_transcript(
    session_key: Option<String>,
) -> Vec<crate::skilllite_bridge::TranscriptMessage> {
    let key = session_key.unwrap_or_else(|| "default".to_string());
    tauri::async_runtime::spawn_blocking(move || crate::skilllite_bridge::load_transcript(&key))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn skilllite_followup_suggestions(
    app: tauri::AppHandle,
    transcript: String,
    workspace: Option<String>,
    config: Option<crate::skilllite_bridge::ChatConfigOverrides>,
) -> crate::skilllite_bridge::LlmInvokeResult<Vec<String>> {
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    match crate::skilllite_bridge::followup_chat_suggestions(transcript, workspace, config, &path)
        .await
    {
        Ok(rows) => crate::skilllite_bridge::LlmInvokeResult::ok(rows),
        Err(err) => crate::skilllite_bridge::LlmInvokeResult::err(
            crate::skilllite_bridge::classify_llm_routing_error_message(&err),
        ),
    }
}

#[tauri::command]
pub async fn skilllite_clear_transcript(
    app: tauri::AppHandle,
    session_key: Option<String>,
    workspace: Option<String>,
) -> Result<(), String> {
    let key = session_key.unwrap_or_else(|| "default".to_string());
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    let path = crate::skilllite_bridge::resolve_skilllite_path_app(&app);
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::clear_transcript(&key, &ws, &path)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(std::convert::identity)
}

#[tauri::command]
pub async fn skilllite_read_memory_file(relative_path: String) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_memory_file(&path)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_read_log_file(filename: String) -> Result<String, String> {
    let name = filename.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_log_file(&name)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_read_output_file(
    relative_path: String,
    workspace: Option<String>,
) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_output_file(&path, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_read_output_file_base64(
    relative_path: String,
    workspace: Option<String>,
) -> Result<String, String> {
    let path = relative_path.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::read_output_file_base64(&path, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_open_directory(
    module: String,
    workspace: Option<String>,
) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::open_directory(&module, workspace)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_reveal_in_file_manager(path: String) -> Result<(), String> {
    match tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::reveal_in_file_manager(&path)
    })
    .await
    {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn skilllite_open_skill_directory(
    workspace: Option<String>,
    skill_name: String,
) -> Result<(), String> {
    let ws = workspace.unwrap_or_else(|| ".".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        crate::skilllite_bridge::open_skill_directory(&ws, &skill_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Read a user-picked image from disk (after native file dialog) for chat attachments.
#[tauri::command]
pub fn skilllite_read_local_image_b64(
    path: String,
) -> Result<crate::skilllite_bridge::ChatImageAttachment, String> {
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

    Ok(crate::skilllite_bridge::ChatImageAttachment {
        media_type: media_type.to_string(),
        data_base64,
    })
}
