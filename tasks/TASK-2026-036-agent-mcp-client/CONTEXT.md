# Context

## Implementation notes

- `types/mcp_servers.rs`: `McpServerEntry`, `parse_mcp_servers_json`.
- `mcp_client/stdio.rs`: tokio child, JSON-RPC line protocol, `initialize` → `notifications/initialized` → `tools/list` / `tools/call`.
- `ExtensionRegistry`: optional `Arc<McpRuntime>`; `ToolHandler::Mcp`.
- Read-only agent mode filters MCP tools via `ProcessExec` capability policy.

## Compatibility

- RPC `config.mcp_servers` overrides merge into `AgentConfig`; desktop also sets `SKILLLITE_MCP_SERVERS_JSON` for child `from_env()`.
