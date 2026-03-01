//! delegate_to_swarm: delegate task to P2P swarm when local capabilities insufficient.
//!
//! §3.4: Only attempts when SKILLLITE_SWARM_URL is set; 5s timeout; graceful fallback on failure.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{ToolDefinition, FunctionDef};
use skilllite_core::protocol::{NodeContext, NodeResult, NodeTask};

pub const SWARM_URL_ENV: &str = "SKILLLITE_SWARM_URL";
const DELEGATE_TIMEOUT_SECS: u64 = 5;

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDef {
            name: "delegate_to_swarm".to_string(),
            description: "Delegate a sub-task to the P2P swarm when local capabilities are insufficient. Requires SKILLLITE_SWARM_URL to be set (e.g. http://192.168.1.10:7700). Returns the remote node's result or an error if swarm is unavailable. 5s timeout.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "Natural-language description of the task to delegate"
                    },
                    "workspace": {
                        "type": "string",
                        "description": "Workspace path (default: current agent workspace)"
                    },
                    "session_key": {
                        "type": "string",
                        "description": "Session key for continuity (default: default)"
                    },
                    "required_capabilities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Capability tags the task requires (e.g. [\"python\", \"web\"])"
                    }
                },
                "required": ["description"]
            }),
        },
    }]
}

pub(super) async fn execute_delegate_to_swarm(
    args: &Value,
    workspace: &Path,
) -> Result<String> {
    let url = match std::env::var(SWARM_URL_ENV) {
        Ok(u) if !u.is_empty() => u,
        _ => {
            return Ok("Swarm not configured: SKILLLITE_SWARM_URL not set. Skipping delegation.".to_string());
        }
    };

    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("description required"))?
        .to_string();

    let workspace_str = args
        .get("workspace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| workspace.to_string_lossy().to_string());

    let session_key = args
        .get("session_key")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let required_capabilities: Vec<String> = args
        .get("required_capabilities")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let task = NodeTask {
        id: uuid::Uuid::new_v4().to_string(),
        description: description.clone(),
        context: NodeContext {
            workspace: workspace_str,
            session_key,
            required_capabilities,
        },
        tool_hint: None,
    };

    let task_url = format!("{}/task", url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    match client
        .post(&task_url)
        .json(&task)
        .timeout(std::time::Duration::from_secs(DELEGATE_TIMEOUT_SECS))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<NodeResult>().await {
                Ok(result) => Ok(format!(
                    "Delegation succeeded.\nResponse: {}\nTask completed: {}",
                    result.response, result.task_completed
                )),
                Err(e) => Ok(format!(
                    "Delegation returned invalid response: {}. Fallback to local execution.",
                    e
                )),
            }
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Ok(format!(
                "Swarm returned error ({}): {}. Fallback to local execution.",
                status, body
            ))
        }
        Err(e) => Ok(format!(
            "Swarm delegation failed (timeout or connection error): {}. Fallback to local execution.",
            e
        )),
    }
}
