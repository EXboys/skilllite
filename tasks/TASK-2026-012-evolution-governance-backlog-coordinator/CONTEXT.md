# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-evolution/src/lib.rs`
  - `crates/skilllite-evolution/src/feedback.rs`
  - `crates/skilllite-agent/src/chat_session.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-core/src/config/env_keys.rs`
- Current behavior:
  - `run_evolution` computes scope and can execute directly.
  - Trigger sources (periodic and decision-count) both call the same evolution entry.
  - No dedicated proposal backlog/ROI arbitration layer exists.

## Architecture Fit

- Layer boundaries involved:
  - Governance remains inside `skilllite-evolution`.
  - `skilllite-agent` and `skilllite-commands` consume unchanged high-level API (`run_evolution`).
- Interfaces to preserve:
  - `run_evolution` function signature and return enum compatibility where possible.
  - Existing feedback DB initialization entry points.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Additive DB schema migration (`CREATE TABLE IF NOT EXISTS`) only.
  - Existing manual CLI run remains supported.

## Design Decisions

- Decision: Keep governance MVP in evolution crate and avoid cross-crate protocol churn.
  - Rationale: Lowest integration risk and fastest path to "proposal split, execution centralized".
  - Alternatives considered: New coordinator crate or event-bus orchestration.
  - Why rejected: Higher complexity and no immediate benefit for P7-C MVP.
- Decision: Default shadow mode with explicit low-risk auto-exec opt-in.
  - Rationale: Safe rollout while collecting backlog/ROI telemetry.
  - Alternatives considered: Keep current auto-exec default and monitor.
  - Why rejected: Does not solve conflict/governance risk by default.

## Open Questions

- [ ] Should medium-risk proposals require an approval callback in a later milestone?
- [ ] Should backlog records be exposed via a dedicated CLI query command in P7-C+1?
