//! Follow-up suggestions via `skilllite suggest-followup --json` (L2).

use std::process::{Command, Stdio};

use crate::skilllite_bridge::chat::{merge_dotenv_with_chat_overrides, ChatConfigOverrides};
use crate::skilllite_bridge::local::engine_types::SuggestFollowupResponse;
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};

pub async fn followup_chat_suggestions(
    transcript: String,
    workspace: Option<String>,
    config_overrides: Option<ChatConfigOverrides>,
    skilllite_path: &std::path::Path,
) -> Result<Vec<String>, String> {
    let trimmed = transcript.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    let raw_workspace = workspace
        .or_else(|| config_overrides.as_ref().and_then(|c| c.workspace.clone()))
        .unwrap_or_else(|| ".".to_string());
    let root = find_project_root(&raw_workspace);
    let mut cmd = Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.args([
        "suggest-followup",
        "--json",
        "--workspace",
        &raw_workspace,
        "--transcript",
        trimmed,
    ]);
    let env_pairs = merge_dotenv_with_chat_overrides(
        load_dotenv_for_child(&raw_workspace),
        config_overrides.as_ref(),
    );
    for (k, v) in env_pairs {
        cmd.env(k, v);
    }
    cmd.current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .map_err(|e| format!("spawn skilllite suggest-followup: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "skilllite suggest-followup failed: {}",
            stderr.trim()
        ));
    }
    let resp: SuggestFollowupResponse =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("parse followup JSON: {}", e))?;
    Ok(resp.suggestions)
}
