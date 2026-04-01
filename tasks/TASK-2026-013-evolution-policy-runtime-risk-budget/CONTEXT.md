# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-evolution/src/lib.rs`
  - `crates/skilllite-core/src/config/env_keys.rs`
  - `docs/en/ENV_REFERENCE.md`
  - `docs/zh/ENV_REFERENCE.md`
- Current behavior:
  - Coordinator selects highest ROI proposal, then applies `force`, `shadow_mode`, and optional
    low-risk auto execute gate. No structured policy decision object exists.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-core` provides env key constants.
  - `skilllite-evolution` performs policy and coordinator decisions.
- Interfaces to preserve:
  - `run_evolution` external behavior.
  - Existing evolution DB schema compatibility.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - New env vars are optional and default to safe behavior.

## Design Decisions

- Decision:
  - Implement policy runtime in `skilllite-evolution` coordinator with reason-chain output and
    risk budget checks based on `evolution_backlog` daily counts.
  - Rationale:
    - Keeps logic local to coordinator, avoids cross-crate boundary changes, and provides immediate
      auditability without external services.
  - Alternatives considered:
    - Add separate policy crate/module with shared governance interfaces.
  - Why rejected:
    - Overkill for current P7-D scope and increases integration complexity.

## Open Questions

- [ ] Should medium/high auto execution be optionally enabled in a later phase?
- [ ] Do we need dedicated backlog columns for policy action/reasons (instead of note text)?
