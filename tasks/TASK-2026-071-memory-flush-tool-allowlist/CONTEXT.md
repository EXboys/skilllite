# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/chat_session.rs` (`run_memory_flush_turn`)
  - `crates/skilllite-agent/src/agent_loop/mod.rs` (`build_registry_with_mcp`)
  - `crates/skilllite-agent/src/extensions/registry.rs` (`CapabilityPolicy`)
  - `crates/skilllite-agent/src/types/config.rs` (`AgentConfig`)
- Current behavior: flush clones session config/skills into a full agent loop with `SilentEventSink`.

## Architecture Fit

- Layer: agent extensions / agent loop only.
- Preserve `EventSink` and memory tool contracts.
- Use capability policy + selective registration rather than ad-hoc name deny lists in the execute path.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility: flush can no longer use non-memory tools; intended security hardening.

## Design Decisions

- Decision: Add `AgentConfig.memory_tools_only` set only by the flush path; registry builder registers memory tools alone under `CapabilityPolicy::memory_flush()`.
  - Rationale: reuses existing policy filtering; empty skills + no MCP closes the remaining holes (untagged read tools / skills).
  - Alternatives considered:
    - Deny-all SilentEventSink confirmations only — insufficient because `write_file` often needs no confirmation.
    - Allowlist at execute time — easier to miss registration paths; policy at build time is stronger.
  - Why rejected: confirmation-only fixes leave unconfirmed mutators open.

## Open Questions

- [x] Should swarm `run_single_task` get the same restriction? No — different product path; ConfirmRequired handled by PR #138.
