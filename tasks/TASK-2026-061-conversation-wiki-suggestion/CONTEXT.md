# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/` chat loop and run metadata.
  - `crates/skilllite-commands/src/wiki.rs` Markdown-only wiki commands.
  - `skilllite/src/cli.rs` and `skilllite/src/dispatch/mod.rs`.
  - Desktop/Assistant event surfaces discovered during implementation.
- Current behavior: Wiki writes occur through explicit commands. Chat/Assistant does not yet emit a wiki lesson suggestion after replans or repeated tool failures.

## Architecture Fit

- Layer boundaries involved: entry/Desktop/CLI observe agent metadata; commands own wiki filesystem writes.
- Interfaces to preserve: no silent writes from agent loop; no memory/SQLite behavior changes.

## Dependency and Compatibility

- New dependencies: None planned.
- Backward compatibility notes: Additive signals and command/API only.

## Design Decisions

- Decision: Emit structured suggestion facts before any write.
  - Rationale: Desktop and CLI can prompt users consistently, and tests can verify thresholds without parsing prose.
  - Alternatives considered: Directly write wiki lessons from chat.
  - Why rejected: User requested prompt plus confirmation, not automatic writes.

- Decision: Store confirmed lessons as raw wiki Markdown and reuse compile.
  - Rationale: Keeps Repo Wiki Markdown-only and uses existing dynamic refresh path.
  - Alternatives considered: Store lessons in memory or SQLite.
  - Why rejected: This feature is project wiki knowledge, not global memory.

## Open Questions

- [ ] Exact Desktop Assistant UI affordance can be refined after the structured payload exists.
