# Technical Context

## Current State

- Relevant crates/files:
- `crates/skilllite-evolution/src/lib.rs`
- `crates/skilllite-core/src/config/env_keys.rs`
- `docs/en/ENV_REFERENCE.md`
- `docs/zh/ENV_REFERENCE.md`
- Current behavior:
  - Acceptance thresholds are compile-time constants in evolution lib.
  - Rollback restores prompt snapshot only (`prompts/_versions/<txn>`).

## Architecture Fit

- Layer boundaries involved:
  - Env key definitions live in `skilllite-core`; runtime usage remains in `skilllite-evolution`.
- Interfaces to preserve:
  - `run_evolution` external behavior and CLI contracts.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Unset new env vars should produce identical behavior to current defaults.

## Design Decisions

- Decision:
  - Introduce acceptance threshold loader from env with current constants as defaults.
  - Extend snapshot/restore helpers to include memory knowledge and skills `_evolved` tree.
  - Rationale:
    - Keeps policy tuning operationally flexible while preserving deterministic defaults.
    - Aligns rollback boundary with real mutation surface.
  - Alternatives considered:
    - Keep constants and expose threshold tuning only via code change.
    - Disable auto-rollback for non-prompt changes.
  - Why rejected:
    - Code-only tuning is too rigid for deployment.
    - Disabling rollback weakens safety guarantees.

## Open Questions

- [ ] Should acceptance thresholds become policy-runtime configurable per risk tier?
- [ ] Should snapshot include additional evolution artifacts beyond `_evolved` in future?
