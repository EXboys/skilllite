# TASK Card

## Metadata

- Task ID: `TASK-2026-016`
- Title: Evolution: parameterize acceptance thresholds and extend rollback boundary
- Status: `done`
- Priority: `P0`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

Current acceptance auto-link uses hard-coded thresholds, making policy tuning difficult across
environments. In addition, auto-rollback restores prompt files only, so memory/skills changes can
drift from rollback intent when degradation is detected.

## Scope

- In scope:
- Parameterize acceptance-window thresholds via env keys.
- Extend evolution rollback snapshot/restore coverage to include memory knowledge and evolved skills.
- Update EN/ZH env reference docs for new variables.
- Add regression tests for threshold env parsing and extended snapshot restore.
- Out of scope:
- Redesigning policy runtime action model.
- Full historical migration of older rollback snapshots.

## Acceptance Criteria

- [x] Acceptance auto-link thresholds are read from env with safe defaults.
- [x] Auto-rollback restores prompts + memory knowledge + evolved skills snapshot (when present).
- [x] EN/ZH env reference docs include new threshold variables.
- [x] `cargo test -p skilllite-evolution` passes with new tests.

## Risks

- Risk:
  - Snapshot size may increase when skills tree is large.
  - Impact:
    - Rollback runtime and disk usage could rise.
  - Mitigation:
    - Keep existing snapshot retention pruning and only snapshot bounded `_evolved` subtree.

## Validation Plan

- Required tests:
- `cargo test -p skilllite-evolution`
- Commands to run:
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p skilllite-evolution`
- Manual checks:
- Configure custom acceptance env thresholds and verify backlog status changes accordingly.
- Simulate rollback path and verify memory/skills artifacts are restored.

## Regression Scope

- Areas likely affected:
- `crates/skilllite-evolution/src/lib.rs`
- `crates/skilllite-core/src/config/env_keys.rs`
- `docs/en/ENV_REFERENCE.md`
- `docs/zh/ENV_REFERENCE.md`
- Explicit non-goals:
- No change to coordinator proposal ordering.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section `15.5.2`
- Related PRs/issues:
- Related docs:
