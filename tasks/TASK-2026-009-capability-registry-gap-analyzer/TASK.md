# TASK Card

## Metadata

- Task ID: `TASK-2026-009`
- Title: Add capability registry and gap analyzer for planning
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors: `Cursor agent`
- Created: `2026-04-01`
- Target milestone:

## Problem

The agent can execute tools and track outcomes, but it cannot explicitly answer what capabilities it currently has, what is missing for a goal, and how large the gap is.

## Scope

- In scope:
  - Add a `CapabilityRegistry` module in `skilllite-agent` to summarize available capabilities.
  - Add a `CapabilityGapAnalyzer` module to compare goal requirements vs current capabilities.
  - Inject capability map and gap report into planning input.
  - Add focused unit tests for registry/gap analysis and planning integration behavior.
- Out of scope:
  - Automated self-repair execution and evolution backlog orchestration.
  - Cross-crate architecture changes.

## Acceptance Criteria

- [x] Planning input includes a compact capability registry summary.
- [x] Planning input includes a capability gap report with required/covered/missing domains and gap ratio.
- [x] New tests cover at least one happy path and one missing-capability path.

## Risks

- Risk: Domain inference heuristics may misclassify intent
  - Impact: Planner may receive noisy capability requirements
  - Mitigation: Keep deterministic keyword mapping, limit injected verbosity, and provide fallback-safe behavior

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - workspace baseline (`fmt`, `clippy`, `test`)
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test`
- Manual checks:
  - Verify planning content includes both capability registry and gap report blocks when applicable.

## Regression Scope

- Areas likely affected:
  - `skilllite-agent` planning prompt assembly and pre-planning analysis flow.
- Explicit non-goals:
  - No autonomous mutation of skills/rules/memory in this task.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section 15.2 and 15.4
- Related PRs/issues:
- Related docs:
  - `spec/task-artifact-language.md`
