# TASK Card

## Metadata

- Task ID: `TASK-2026-015`
- Title: Evolution: auto-link acceptance status to metrics window
- Status: `done`
- Priority: `P0`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

Evolution proposals currently move to `executed` with `acceptance_status=pending_validation`, but
there is no automatic judgement that links acceptance outcome to the core metrics window. This
keeps acceptance lifecycle partially manual and weakens governance completeness.

## Scope

- In scope:
- Add automatic acceptance evaluation for executed backlog proposals.
- Evaluate against a metrics window using `first_success_rate`, `user_correction_rate`, and
  rollback rate.
- Update `evolution_backlog.acceptance_status` and note with deterministic summary.
- Add regression tests for met / not_met / pending_validation paths.
- Out of scope:
- Redesigning evolution metric definitions.
- Dashboard UI changes for acceptance visualization.

## Acceptance Criteria

- [x] After proposal execution, acceptance status is auto-evaluated from metrics window.
- [x] Acceptance uses the three required signals: success rate, correction rate, rollback rate.
- [x] Backlog note includes machine-generated judgement summary for auditability.
- [x] `cargo test -p skilllite-evolution` passes with new regression tests.

## Risks

- Risk:
  - Thresholds may be too strict/loose and cause misclassification.
  - Impact:
    - Good proposals may remain pending too long or be marked not met too early.
  - Mitigation:
    - Use conservative defaults, require minimum window sample, and expose clear note summary.

## Validation Plan

- Required tests:
- `cargo test -p skilllite-evolution`
- Commands to run:
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p skilllite-evolution`
- Manual checks:
- Run `skilllite evolution status` and `skilllite evolution backlog` to verify acceptance statuses
  update with judgement note.

## Regression Scope

- Areas likely affected:
- `crates/skilllite-evolution/src/lib.rs`
- Explicit non-goals:
- No changes to proposal generation heuristics.
- No changes to sandbox or skill confirmation flow.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section `15.5.2` lines `2727-2729`
- Related PRs/issues:
- Related docs:
