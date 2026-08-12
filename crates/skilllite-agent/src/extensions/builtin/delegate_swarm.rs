//! delegate_to_swarm: delegate task to P2P swarm when local capabilities insufficient.
//!
//! §3.4: Only attempts when SKILLLITE_SWARM_URL is set; 5s timeout; graceful fallback on failure.
//!
//! Security: the delegated `NodeTask.context.workspace` is always the calling agent's
//! workspace. LLM-supplied `workspace` arguments are ignored so a model cannot retarget
//! local swarm execution (and `write_file` containment) to an arbitrary filesystem root.

use crate::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::types::{EventSink, FunctionDef, ToolDefinition};
use skilllite_core::config::env_keys::swarm;
use skilllite_core::protocol::{NodeContext, NodeResult, NodeTask};

pub const SWARM_URL_ENV: &str = swarm::SKILLLITE_SWARM_URL;
const DELEGATE_TIMEOUT_SECS: u64 = 5;

pub(super) fn tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDef {
            name: "delegate_to_swarm".to_string(),
            description: "Delegate a sub-task to the P2P swarm when local capabilities are insufficient. Requires SKILLLITE_SWARM_URL (e.g. http://127.0.0.1:7700). If the swarm uses SKILLLITE_SWARM_TOKEN, set the same value in the environment so requests send Authorization: Bearer. 5s timeout. Execution always uses the current agent workspace (caller-supplied workspace overrides are ignored).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "Natural-language description of the task to delegate"
                    },
                    "workspace": {
                        "type": "string",
                        "description": "Ignored for security; delegated tasks always use the current agent workspace"
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

/// Resolve the workspace string embedded in a delegated [`NodeTask`].
///
/// Always returns the calling agent's workspace. Any LLM-provided `workspace`
/// argument is ignored (and logged when it would have retargeted execution).
pub(super) fn effective_delegate_workspace(args: &Value, agent_workspace: &Path) -> String {
    let agent = agent_workspace.to_string_lossy().to_string();
    if let Some(requested) = args.get("workspace").and_then(|v| v.as_str()) {
        let trimmed = requested.trim();
        if !trimmed.is_empty() && trimmed != agent {
            tracing::warn!(
                requested_workspace = %trimmed,
                agent_workspace = %agent,
                "Ignoring delegate_to_swarm workspace override; using agent workspace"
            );
        }
    }
    agent
}

pub(super) async fn execute_delegate_to_swarm(
    args: &Value,
    workspace: &Path,
    event_sink: &mut dyn EventSink,
) -> Result<String> {
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::Error::validation("description required"))?
        .to_string();

    let url = match std::env::var(SWARM_URL_ENV) {
        Ok(u) if !u.is_empty() => u,
        _ => {
            let msg = "Swarm not configured: SKILLLITE_SWARM_URL not set. Skipping delegation.";
            event_sink.on_swarm_failed(msg);
            return Ok(msg.to_string());
        }
    };
    event_sink.on_swarm_started(&description);

    let workspace_str = effective_delegate_workspace(args, workspace);

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
    event_sink.on_swarm_progress("submitting task to swarm");

    skilllite_core::config::load_dotenv();
    let mut req = client
        .post(&task_url)
        .json(&task)
        .timeout(std::time::Duration::from_secs(DELEGATE_TIMEOUT_SECS));
    if let Ok(tok) = std::env::var(swarm::SKILLLITE_SWARM_TOKEN) {
        let t = tok.trim();
        if !t.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", t));
        }
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<NodeResult>().await {
            Ok(result) => {
                let summary = if result.response.is_empty() {
                    format!("remote task completed={}", result.task_completed)
                } else {
                    format!(
                        "remote task completed={} response={}",
                        result.task_completed, result.response
                    )
                };
                event_sink.on_swarm_finished(&summary);
                Ok(format!(
                    "Delegation succeeded.\nResponse: {}\nTask completed: {}",
                    result.response, result.task_completed
                ))
            }
            Err(e) => {
                let msg = format!(
                    "Delegation returned invalid response: {}. Fallback to local execution.",
                    e
                );
                event_sink.on_swarm_failed(&msg);
                Ok(msg)
            }
        },
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let msg = format!(
                "Swarm returned error ({}): {}. Fallback to local execution.",
                status, body
            );
            event_sink.on_swarm_failed(&msg);
            Ok(msg)
        }
        Err(e) => {
            let msg = format!(
                "Swarm delegation failed (timeout or connection error): {}. Fallback to local execution.",
                e
            );
            event_sink.on_swarm_failed(&msg);
            Ok(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn effective_delegate_workspace_ignores_llm_override() {
        let agent = PathBuf::from("/home/user/project");
        let args = json!({
            "description": "write a file",
            "workspace": "/tmp/attacker_ws"
        });
        assert_eq!(
            effective_delegate_workspace(&args, &agent),
            "/home/user/project"
        );
    }

    #[test]
    fn effective_delegate_workspace_defaults_to_agent_when_omitted() {
        let agent = PathBuf::from("/home/user/project");
        let args = json!({ "description": "summarize" });
        assert_eq!(
            effective_delegate_workspace(&args, &agent),
            "/home/user/project"
        );
    }

    #[test]
    fn effective_delegate_workspace_preserves_matching_agent_path() {
        let agent = PathBuf::from("/home/user/project");
        let args = json!({
            "description": "ok",
            "workspace": "/home/user/project"
        });
        assert_eq!(
            effective_delegate_workspace(&args, &agent),
            "/home/user/project"
        );
    }
}
