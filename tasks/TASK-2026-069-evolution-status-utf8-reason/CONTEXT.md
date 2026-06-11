# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-commands/src/evolution_status.rs`.
- Current behavior: The JSON status path serializes full event reasons safely, but the human status path computes `reason_short` with `&reason[..47]` when `reason.len() > 50`.

## Architecture Fit

- Layer boundaries involved: CLI entry calls `skilllite-commands`; persistence remains in `skilllite-evolution`.
- Interfaces to preserve: `cmd_status(json, workspace, periodic_anchor_unix)` and `EvolutionStatusSnapshot` shape.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: No schema, command flag, or JSON contract changes.

## Design Decisions

- Decision: Add a local `shorten_status_reason` helper that checks character count and builds the preview with `.chars().take(...)`.
  - Rationale: This directly removes the panic source while preserving the existing output intent.
  - Alternatives considered: Reuse `evolution.rs` private `truncate_chars` or make a shared utility.
  - Why rejected: Sharing would broaden the edit beyond the critical path and require extra module surface changes.

## Open Questions

- [x] Does the fix need docs updates? No; behavior is a crash fix with unchanged user-facing command semantics.
- [x] Does the regression need LLM/API credentials? No; it seeds the local SQLite event log directly.
