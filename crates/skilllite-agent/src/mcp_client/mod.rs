//! Outbound MCP (Model Context Protocol) client for the agent.
//!
//! Connects to configured stdio MCP servers, merges `tools/list` into the agent tool registry,
//! and forwards `tools/call` during [`crate::extensions::ExtensionRegistry::execute`].

mod bootstrap;
mod runtime;
mod stdio;

pub use crate::types::{parse_mcp_servers_json, McpServerEntry};
pub use bootstrap::{bootstrap_mcp, McpBootstrap};
pub use runtime::McpRuntime;
