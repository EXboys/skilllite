# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-executor/src/memory.rs` (`index_path`)
  - `crates/skilllite-executor/src/rpc.rs` (`handle_memory_write` / `handle_memory_search`)
  - `crates/skilllite-agent/src/extensions/memory.rs`
  - `crates/skilllite-agent/src/chat_session.rs`
  - `crates/skilllite-core/src/path_validation.rs`
- Current behavior: `workspace_root.join("memory").join(format!("{agent_id}.sqlite"))` with no validation.

## Architecture Fit

- Layer boundaries involved: core path validation + executor memory store + agent memory tools
- Interfaces to preserve: memory RPC method names and happy-path payload shape for valid IDs

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: only invalid/escaping IDs start failing

## Design Decisions

- Decision: validate inside `index_path` (return `Result`) so every caller is covered, including session-key-as-agent-id indexing.
- Decision: mirror single-segment rules used for session keys / skill dir names; keep a dedicated `InvalidAgentId` error variant.
