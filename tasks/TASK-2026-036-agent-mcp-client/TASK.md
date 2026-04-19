# TASK Card

## Metadata

- Task ID: `TASK-2026-036`
- Title: Agent MCP client (stdio, enable/disable)
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-19`
- Target milestone:

## Problem

The agent could not act as an MCP **client** to connect external stdio MCP servers, so tools from other hosts (e.g. community MCP servers) were unavailable in `agent-rpc` / desktop chat.

## Scope

- In scope:
  - `crates/skilllite-agent`: `mcp_client/` (stdio JSON-RPC), `bootstrap_mcp`, `ExtensionRegistry` + `ToolHandler::Mcp`, `AgentConfig.mcp_servers`, env `SKILLLITE_MCP_SERVERS_JSON`, `SKILLLITE_AGENT_MCP_CLIENT`, RPC `config.mcp_servers` merge.
  - Desktop: settings UI (add/remove rows, per-row enable), bridge env + JSON config.
  - Docs: ENV_REFERENCE EN/ZH, ARCHITECTURE EN/ZH.
- Out of scope:
  - MCP over HTTP/SSE transports.
  - Sampling / reverse LLM requests from MCP servers.

## Acceptance Criteria

- [x] Agent discovers tools via `tools/list` from configured stdio MCP servers and exposes them as `mcp__<id>__<tool>` in the same tool plane as builtins/skills.
- [x] Each server entry can be disabled (`enabled: false`) or omitted; global kill-switch via `SKILLLITE_AGENT_MCP_CLIENT=0`.
- [x] Desktop settings allow adding/removing servers and toggling enable per row; config reaches `agent-rpc`.
- [x] `cargo test -p skilllite-agent` passes; docs EN/ZH updated.

## Risks

- Risk: External MCP tools run with broad effect (similar to `run_command`).
  - Impact: Workspace or network side effects beyond SkillLite sandbox if the remote server is malicious or misconfigured.
  - Mitigation: Tools mapped to `ProcessExec`; excluded under `read_only_tools`; document env toggles; users configure servers explicitly.

## Validation Plan

- Required tests: existing agent tests + new `types::mcp_servers` parse test + `mcp_client::bootstrap` sanitize test.
- Commands to run: `cargo test -p skilllite-agent`, `cargo check -p skilllite`, `cargo check --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`, `python3 scripts/validate_tasks.py`.
- Manual checks: optional — configure a trivial stdio MCP and verify tool appears in logs (not required for merge).

## Regression Scope

- Areas likely affected: `agent_loop` registry construction, `ExtensionRegistry`, `rpc` config merge, desktop chat env/JSON, settings store.
- Explicit non-goals: Changing `skilllite mcp` server behavior.

## Links

- Related docs: `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.
