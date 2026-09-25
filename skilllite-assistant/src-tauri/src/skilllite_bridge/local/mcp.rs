//! MCP server entry shape for `SKILLLITE_MCP_SERVERS_JSON` (matches agent-rpc env).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerEntry {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

fn default_true() -> bool {
    true
}
