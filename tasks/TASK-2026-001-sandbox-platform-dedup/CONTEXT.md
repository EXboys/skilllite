# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-sandbox/src/macos.rs`
  - `crates/skilllite-sandbox/src/linux.rs`
  - `crates/skilllite-sandbox/src/windows.rs`
- Current behavior:
  - Per-platform execution paths include overlapping control flow.

## Architecture Fit

- Layer boundaries involved:
  - sandbox must stay below commands/agent in dependency graph.
- Interfaces to preserve:
  - public runner API and config semantics.

## Dependency and Compatibility

- New dependencies:
  - none planned.
- Backward compatibility notes:
  - preserve env variables and fallback semantics.

## Design Decisions

- Decision: extract common orchestration into shared helper/trait layer.
  - Rationale: reduce duplicate code and improve parity.
  - Alternatives considered: keep per-platform copy with comments.
  - Why rejected: duplicate maintenance cost remains high.

## Open Questions

- [ ] Should parity tests be table-driven across all backends?
- [ ] Do we need explicit feature flags for any shared helper?
