# TASK Card

## Metadata

- Task ID: `TASK-2026-008`
- Title: Implement Goal Contract extraction for planning
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors: `Cursor agent`
- Created: `2026-04-01`
- Target milestone:

## Problem

The current planning stage only extracts `GoalBoundaries` (scope/exclusions/completion), but lacks an executable contract layer. As a result, acceptance criteria, deadlines, and risk levels in user goals cannot be consistently structured and used.

## Scope

- In scope:
  - Add a `goal_contract` module and data structures in `skilllite-agent`.
  - Support extracting goal, acceptance, constraints, deadline, and risk level from user goal text.
  - Inject extracted results into planning input (`planning user content`) to guide task decomposition.
  - Add unit tests covering success and failure/empty-input paths.
- Out of scope:
  - Do not implement later P7 modules such as capability gap analyzer or env profiler.
  - Do not change external CLI arguments or protocols.

## Acceptance Criteria

- [x] `GoalContract` includes and exposes `goal/acceptance/constraints/deadline/risk_level`.
- [x] Planning injects a `Goal Contract` block when data exists, and injects nothing when empty.
- [x] New extraction logic includes at least one happy-path and one failure/empty-path test.

## Risks

- Risk: Rule-based extraction may misclassify and produce an incorrect contract
  - Impact: Planning may drift or become over-constrained
  - Mitigation: Inject only non-empty fields; keep original user request as PRIMARY; add regression tests

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - Workspace baseline checks: fmt/clippy/test
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test`
- Manual checks:
  - Verify planning injection text contains the contract block with correct field names

## Regression Scope

- Areas likely affected:
  - Planning-stage prompt injection logic in `skilllite-agent`
  - Goal extraction helpers (existing mixed extraction flow around goal boundaries)
- Explicit non-goals:
  - No new crate introduction and no cross-layer dependency adjustments

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` 15.4 `goal_contract`
- Related PRs/issues:
- Related docs:
  - `docs/zh/README.md` (if shipped-module notes need to be expanded)
