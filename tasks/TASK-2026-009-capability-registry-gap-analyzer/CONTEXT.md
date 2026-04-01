# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/task_planner.rs`
  - `crates/skilllite-agent/src/agent_loop/planning.rs`
  - `crates/skilllite-agent/src/skills/mod.rs`
  - `crates/skilllite-agent/src/extensions/registry.rs`
- Current behavior:
  - Planner receives user request, goal boundaries, and goal contract.
  - Skills are listed, but capability coverage and gap quantification are not explicit.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-agent` internal planning modules only.
- Interfaces to preserve:
  - Existing planner signatures and fallback-safe planning behavior.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Prompt additions are additive; no protocol breaking changes.

## Design Decisions

- Decision:
  - Introduce deterministic capability domain inference from skill metadata and goal text.
  - Rationale:
    - Lightweight and predictable, suitable for MVP capability awareness.
  - Alternatives considered:
    - LLM-only capability gap analysis.
  - Why rejected:
    - Adds runtime variance and extra cost for baseline capability mapping.

## Open Questions

- [ ] Should usage statistics be incorporated into capability level scoring in a follow-up?
- [ ] Should gap severity feed directly into runtime risk gating?
