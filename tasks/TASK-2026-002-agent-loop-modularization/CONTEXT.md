# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/agent_loop/mod.rs`
  - `crates/skilllite-agent/src/chat_session.rs`
- Current behavior:
  - Main loop coordinates LLM response parsing, tool execution, and reflection.

## Architecture Fit

- Layer boundaries involved:
  - agent remains above executor/sandbox.
- Interfaces to preserve:
  - extension registry and tool call contracts.

## Dependency and Compatibility

- New dependencies:
  - none planned.
- Backward compatibility notes:
  - maintain current event and response behavior.

## Design Decisions

- Decision: split branch-heavy methods into module-level handlers.
  - Rationale: lower cognitive complexity and enable targeted tests.
  - Alternatives considered: only comments and internal sections.
  - Why rejected: readability and testability gains are limited.

## Open Questions

- [ ] Which boundaries should stay in `mod.rs` vs handler modules?
- [ ] Should reflection remain colocated or become separate module entry?
