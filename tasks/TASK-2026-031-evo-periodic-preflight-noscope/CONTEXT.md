# Technical Context

## Current State

- Relevant crates/files: `skilllite-evolution` (`growth_schedule.rs`, `scope.rs`, `run.rs`, `lib.rs`), `skilllite-agent` (`chat_session.rs`), `skilllite-assistant` (`integrations.rs`, `life_pulse.rs`), frontend `evolutionDisplay.ts`, i18n.
- Previous behavior: `growth_due` returned `bool`; periodic arm could be true with zero proposals, always calling `run_evolution` and logging generic NoScope.

## Architecture Fit

- Layer boundaries: Evolution core owns schedule + proposal predicates; agent and desktop are thin callers.
- Interfaces to preserve: `signal_burst_due` unchanged; `run_evolution` semantics unchanged except log strings when proposals empty.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility: Old log rows keep legacy reason string mapping in UI.

## Design Decisions

- Decision: `GrowthDueOutcome { due, periodic_only }` instead of a second query.
  - Rationale: Single evaluation keeps anchor mutation consistent.
  - Alternatives considered: Separate `would_periodic_fire` helper — duplicated anchor logic.
  - Why rejected: Error-prone drift.

- Decision: Fail-open `would_have_evolution_proposals` on DB error (`unwrap_or(true)` desktop, agent).
  - Rationale: Avoid missing evolution on transient failures.
  - Alternatives considered: Fail-closed — risks silent stall.
  - Why rejected: Safety for evolution cadence.

## Open Questions

- [x] None remaining for this task.
