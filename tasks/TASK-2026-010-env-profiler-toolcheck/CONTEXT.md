# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/task_planner.rs`
  - `crates/skilllite-agent/src/capability_registry.rs`
  - `crates/skilllite-agent/src/capability_gap_analyzer.rs`
- Current behavior:
  - Planning receives goal/contract/capability context but not local runtime tool readiness.

## Architecture Fit

- Layer boundaries involved:
  - Internal `skilllite-agent` planning path only.
- Interfaces to preserve:
  - Existing planner flow and message construction contract.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Additive prompt block; existing behavior remains unchanged when profile is empty.

## Design Decisions

- Decision:
  - Use fixed allowlist checks for `git/python/node/npm/cargo` with `--version`.
  - Rationale:
    - Low risk, low noise, and predictable behavior across environments.
  - Alternatives considered:
    - Broader host probing or dynamic command discovery.
  - Why rejected:
    - Higher risk and potential sensitivity concerns for endpoint security.

## Open Questions

- [ ] Should profile results be cached per session to avoid repeated probes?
- [ ] Should missing critical tools be surfaced as explicit gap dimensions in analyzer scoring?
