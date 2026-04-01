# Technical Context

## Current State

- Relevant crates/files:
- `crates/skilllite-evolution/src/lib.rs`
- `crates/skilllite-evolution/src/feedback.rs`
- Current behavior:
  - Proposal is moved to `executed` with `acceptance_status=pending_validation` after run.
  - No automatic post-window linkage to acceptance outcome exists.

## Architecture Fit

- Layer boundaries involved:
  - Keep acceptance evaluation inside `skilllite-evolution` where coordinator/backlog lifecycle
    already lives.
- Interfaces to preserve:
  - `run_evolution` return contract.
  - Existing `evolution_backlog` schema fields and CLI readers.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Existing statuses remain valid; auto-link only refines acceptance lifecycle.

## Design Decisions

- Decision:
  - Add deterministic acceptance-window evaluator and call it after `executed` update.
  - Rationale:
    - Keeps proposal governance and acceptance judgement in one module with shared DB context.
  - Alternatives considered:
    - Evaluate in CLI layer on read.
    - External scheduled job for acceptance synchronization.
  - Why rejected:
    - CLI-read evaluation is not durable in DB state.
    - External job adds operational complexity for a core lifecycle step.

## Open Questions

- [ ] Should thresholds be env-configurable in a later phase?
- [ ] Should acceptance judgement history become a dedicated table?
