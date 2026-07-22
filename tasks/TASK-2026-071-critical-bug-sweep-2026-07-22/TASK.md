# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Critical Bug Sweep 2026-07-22
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-07-22`
- Target milestone:

## Problem

Recent changes may contain correctness or security regressions that escaped review. This
sweep determines whether commits merged after the previous recorded sweep introduce a
concrete high-severity failure.

## Scope

- In scope:
  - Commits after merge `74f8417` through `origin/main` at `12010e8`.
  - Full caller and downstream tracing for modified Rust code and dependencies.
  - Minimal fixes and regression tests only if a critical bug is confirmed.
- Out of scope:
  - Style issues, low-severity edge cases, and theoretical concerns without a trigger.
  - Unmerged branches and unrelated pre-existing behavior.

## Acceptance Criteria

- [x] Review every behavioral change in the selected commit range.
- [x] Record a concrete trigger and impact for every surfaced finding.
- [x] Open a fix PR only when a critical bug and minimal high-confidence fix are proven.
- [x] Record actual validation evidence and attempt Slack notification of the outcome.

## Risks

- Risk: A style-only diff can hide a semantic or security regression.
  - Impact: Crash, data loss, sandbox bypass, or significant user-facing breakage.
  - Mitigation: Compare old and new semantics and trace all affected callers.
- Risk: False positives create unnecessary or unsafe fixes.
  - Impact: Regressions and review noise.
  - Mitigation: Require a concrete trigger and independently verifiable evidence.

## Validation Plan

- Required tests: Task artifact validation; targeted package tests for the changed areas;
  workspace formatting, Clippy, and test checks if code is changed.
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo test -p skilllite-core`
  - `cargo test -p skilllite-agent`
  - `cargo check --locked --workspace`
- Manual checks: Inspect commit diffs, type definitions, dependency graph, callers, and
  downstream security behavior.

Validation result: No product code changed. Task validation, affected-crate tests, full
workspace tests, and locked workspace check passed. Strict Clippy was blocked only by
two pre-existing findings outside the reviewed commit range; a run allowing exactly
those two lint categories passed all targets. Slack delivery was attempted for all three
visible channels and rejected because the Cursor bot is not a channel member.

## Regression Scope

- Areas likely affected: sandbox configuration, chat transcript reconstruction, and
  Rayon/crossbeam dependency resolution.
- Explicit non-goals: Broad refactors, unrelated cleanup, and remediation of
  non-critical pre-existing issues.

## Links

- Source TODO section: N/A (scheduled critical bug automation).
- Related PRs/issues: PR #118, PR #119, RUSTSEC-2026-0204.
- Related docs: `spec/verification-integrity.md`,
  `spec/security-nonnegotiables.md`, `spec/testing-policy.md`.
