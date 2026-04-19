# PRD

## Goal

Enable the SkillLite agent to consume **stdio MCP servers** as first-class tools (Cursor-style outbound MCP), with per-server enable/disable and desktop configuration.

## User-visible behavior

- Tools from connected servers appear as `mcp__<alias>__<remote_tool_name>`.
- Settings → Workspace & sandbox: section **Outbound MCP (stdio)** to add/remove entries and toggle each row.
- Environment: `SKILLLITE_MCP_SERVERS_JSON`, `SKILLLITE_AGENT_MCP_CLIENT=0` to disable all.

## N/A

- HTTP/SSE MCP client (future work).
