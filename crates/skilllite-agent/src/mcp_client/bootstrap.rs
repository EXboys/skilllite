//! Discover MCP tools from configured servers and build [`RegisteredTool`] list + shared runtime.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::extensions::{RegisteredTool, ToolCapability, ToolHandler};
use crate::types::{AgentConfig, FunctionDef, ToolDefinition};

use super::runtime::McpRuntime;
use super::stdio::McpStdioSession;
use crate::types::McpServerEntry;

const MAX_MCP_TOOL_NAME_LEN: usize = 96;

/// Result of wiring MCP servers for one agent loop invocation.
pub struct McpBootstrap {
    pub tools: Vec<RegisteredTool>,
    pub runtime: Option<Arc<McpRuntime>>,
}

impl McpBootstrap {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tools: Vec::new(),
            runtime: None,
        }
    }
}

/// Connect enabled MCP servers, run `tools/list`, register prefixed tools.
pub async fn bootstrap_mcp(config: &AgentConfig) -> McpBootstrap {
    if std::env::var("SKILLLITE_AGENT_MCP_CLIENT").ok().as_deref() == Some("0") {
        return McpBootstrap::empty();
    }

    let entries: Vec<&McpServerEntry> = config
        .mcp_servers
        .iter()
        .filter(|e| e.is_usable())
        .collect();

    if entries.is_empty() {
        return McpBootstrap::empty();
    }

    let runtime = Arc::new(McpRuntime::default());
    let mut registered: Vec<RegisteredTool> = Vec::new();

    for entry in entries {
        let sid = sanitize_id(&entry.id);
        if sid.is_empty() {
            tracing::warn!("Skipping MCP server with empty id after sanitize");
            continue;
        }

        match connect_server(entry, config, Arc::clone(&runtime)).await {
            Ok(mut tools) => registered.append(&mut tools),
            Err(e) => tracing::warn!("MCP server '{}' unavailable: {}", entry.id.trim(), e),
        }
    }

    if registered.is_empty() {
        return McpBootstrap::empty();
    }

    McpBootstrap {
        tools: registered,
        runtime: Some(runtime),
    }
}

async fn connect_server(
    entry: &McpServerEntry,
    config: &AgentConfig,
    runtime: Arc<McpRuntime>,
) -> crate::Result<Vec<RegisteredTool>> {
    let session = Arc::new(McpStdioSession::spawn(entry, &config.workspace).await?);
    session.handshake().await?;
    let list_result = session.tools_list().await?;
    let tools_val = list_result
        .get("tools")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let sid = sanitize_id(&entry.id);
    runtime.insert_session(sid.clone(), Arc::clone(&session));

    let mut out = Vec::new();
    for t in tools_val {
        let Some(name) = t.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let desc = t
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("MCP tool");
        let input_schema = t
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}}));

        let prefixed = format!("mcp__{}__{}", sid, sanitize_remote_tool_name(name));
        if prefixed.len() > MAX_MCP_TOOL_NAME_LEN {
            tracing::warn!("Skipping MCP tool '{}': prefixed name too long", name);
            continue;
        }

        let parameters = input_schema_to_parameters(&input_schema);
        let def = ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: prefixed.clone(),
                description: format!("(MCP:{}) {}", sid, desc),
                parameters,
            },
        };

        let handler = ToolHandler::Mcp {
            server_id: sid.clone(),
            remote_tool: name.to_string(),
        };

        out.push(RegisteredTool::new(
            def,
            vec![ToolCapability::ProcessExec],
            handler,
        ));
    }

    Ok(out)
}

fn input_schema_to_parameters(schema: &Value) -> Value {
    if let Some(obj) = schema.as_object() {
        let has_props = obj.contains_key("properties");
        let is_obj = obj.get("type").and_then(Value::as_str) == Some("object");
        if has_props || is_obj {
            return schema.clone();
        }
    }
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

fn sanitize_id(id: &str) -> String {
    let s = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    s.trim_matches('_').to_string()
}

fn sanitize_remote_tool_name(name: &str) -> String {
    let s = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "tool".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_id_keeps_simple() {
        assert_eq!(sanitize_id("my-fs"), "my-fs");
        assert_eq!(sanitize_id("a b"), "a_b");
    }
}
