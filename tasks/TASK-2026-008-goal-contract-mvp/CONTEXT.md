# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/goal_boundaries.rs`
  - `crates/skilllite-agent/src/agent_loop/helpers.rs`
  - `crates/skilllite-agent/src/agent_loop/planning.rs`
- Current behavior:
  - Before planning, Goal Boundaries are extracted (`scope/exclusions/completion_conditions`).
  - There is no goal-contract structure to explicitly represent acceptance/deadline/risk level.

## Architecture Fit

- Layer boundaries involved:
  - Internal module calls inside `skilllite-agent` only, with no cross-crate changes.
- Interfaces to preserve:
  - Keep `run_planning_phase` main-flow signature clear; only add optional injected data.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - No breaking change; behavior remains consistent with prior flow when no contract is available.

## Design Decisions

- Decision: Use heuristic extraction plus structured rendering, aligned with the existing `goal_boundaries` style.
  - Rationale: Simple and stable implementation for MVP landing.
  - Alternatives considered: Direct LLM JSON extraction only.
  - Why rejected: Higher cost and less controllability as a sole strategy for the initial scope.

## Open Questions

- [ ] Should `SKILLLITE_GOAL_CONTRACT_LLM_EXTRACT` be introduced as an optional enhancement toggle?
- [ ] Should `risk_level` be linked to `high_risk` policy as a hard constraint?
