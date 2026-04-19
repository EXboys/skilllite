//! Shared state: one stdio session per configured MCP server id (for the current agent loop).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::Result;

use super::stdio::McpStdioSession;

/// Holds stdio MCP sessions for the current agent loop (one per server `id`).
#[derive(Default)]
pub struct McpRuntime {
    sessions: std::sync::Mutex<HashMap<String, Arc<McpStdioSession>>>,
}

impl std::fmt::Debug for McpRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpRuntime").finish_non_exhaustive()
    }
}

impl McpRuntime {
    pub(crate) fn insert_session(&self, server_id: String, session: Arc<McpStdioSession>) {
        let Ok(mut g) = self.sessions.lock() else {
            tracing::error!("McpRuntime mutex poisoned");
            return;
        };
        g.insert(server_id, session);
    }

    pub(crate) async fn call_tool(
        &self,
        server_id: &str,
        remote_tool: &str,
        arguments: Value,
    ) -> Result<String> {
        let session = {
            let g = self
                .sessions
                .lock()
                .map_err(|_| crate::error::Error::validation("MCP runtime lock poisoned"))?;
            g.get(server_id).cloned()
        };
        let Some(session) = session else {
            return Err(crate::error::Error::validation(format!(
                "No active MCP session for server '{}'",
                server_id
            )));
        };
        let raw = session.tools_call(remote_tool, arguments).await?;
        Ok(format_tools_call_result(&raw))
    }
}

fn format_tools_call_result(v: &Value) -> String {
    let is_err = v.get("isError").and_then(|b| b.as_bool()) == Some(true);
    let mut text = String::new();
    if let Some(parts) = v.get("content").and_then(|c| c.as_array()) {
        for part in parts {
            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = part.get("text").and_then(|x| x.as_str()) {
                    text.push_str(t);
                }
            }
        }
    }
    if text.is_empty() {
        text = v.to_string();
    }
    if is_err {
        format!("MCP tool error: {}", text)
    } else {
        text
    }
}
