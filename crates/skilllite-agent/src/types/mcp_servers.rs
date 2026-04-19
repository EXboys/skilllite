//! Outbound MCP server entries for [`super::AgentConfig`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One stdio-based MCP server the agent may connect to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerEntry {
    /// Stable alias used for tool name prefixes (`mcp__{id}__...`).
    pub id: String,
    /// When false, this server is skipped entirely.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Executable to spawn (e.g. `npx`, `uvx`, or absolute path).
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory for the child process; defaults to agent workspace when unset.
    #[serde(default)]
    pub cwd: Option<String>,
}

fn default_true() -> bool {
    true
}

impl McpServerEntry {
    /// Returns true when this entry should be connected.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.enabled && !self.id.trim().is_empty() && !self.command.trim().is_empty()
    }
}

/// Parse `SKILLLITE_MCP_SERVERS_JSON` into a list of server entries (best-effort).
pub fn parse_mcp_servers_json(raw: &str) -> Vec<McpServerEntry> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    match serde_json::from_str::<Vec<McpServerEntry>>(raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("SKILLLITE_MCP_SERVERS_JSON parse error: {}", e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_entry() {
        let j = r#"[{"id":"fs","enabled":true,"command":"npx","args":["-y","@modelcontextprotocol/server-filesystem","/tmp"]}]"#;
        let v = parse_mcp_servers_json(j);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "fs");
        assert!(v[0].enabled);
    }
}
