# TASK Card

## Metadata

- Task ID: `TASK-2026-012`
- Title: Evolution governance backlog coordinator
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone: `P7-C (2 weeks)`

## Problem

Active and passive evolution currently converge directly into execution, which causes conflict risk, weak prioritization, and poor traceability of why one evolution run was executed over another.

## Scope

- In scope:
  - Introduce a unified `EvolutionProposal` model.
  - Implement an `evolution_backlog` persistence layer in evolution SQLite.
  - Add a coordinator that handles queueing, deduplication, lock, and ROI scoring.
  - Switch current dual trigger paths to "proposal only" and centralize execution decision in coordinator.
  - Start in shadow mode by default and gate auto-execution to low-risk proposals only.
- Out of scope:
  - Full multi-round online learning policy redesign.
  - New external dependencies or service-based scheduler.
  - UI-level backlog dashboard in this task.

## Acceptance Criteria

- [x] Evolution has a unified proposal structure shared by active/passive triggers.
- [x] Coordinator persists proposal backlog records with dedupe key and ROI metadata.
- [x] Shadow mode defaults to enabled so automatic evolution execution is suppressed by default.
- [x] A guarded path exists to auto-execute only low-risk proposals when explicitly enabled.
- [x] Existing evolution CLI/manual forced run path remains functional.

## Risks

- Risk: Shadow mode default may reduce immediate automatic evolution throughput.
  - Impact: Users may perceive fewer autonomous updates.
  - Mitigation: Keep forced/manual run available and expose explicit env toggles.
- Risk: Proposal scoring heuristics may be too naive in MVP.
  - Impact: Suboptimal proposal ordering.
  - Mitigation: Persist scores/inputs for future calibration and keep scoring deterministic.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-evolution`
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks:
  - Verify backlog table records proposal rows.
  - Verify shadow mode path returns without auto execution.
  - Verify forced evolution still executes.

## Regression Scope

- Areas likely affected:
  - Evolution trigger flow (`run_evolution`, periodic/decision-count triggers).
  - CLI output semantics for "nothing to evolve" scenarios.
  - SQLite schema initialization/migration for evolution DB.
- Explicit non-goals:
  - No change to sandbox/security enforcement semantics.
  - No change to tool result dedupe behavior.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section `15.5 P7-C`
- Related PRs/issues:
- Related docs:
  - `docs/en/ENV_REFERENCE.md`
  - `docs/zh/ENV_REFERENCE.md`
