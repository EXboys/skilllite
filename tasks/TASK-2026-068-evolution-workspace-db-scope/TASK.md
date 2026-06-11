# TASK Card

## Metadata

- Task ID: `TASK-2026-068`
- Title: Evolution workspace DB scope
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-10`
- Target milestone:

## Problem

Recent L2 evolution CLI and desktop JSON paths may ignore the `--workspace` flag when opening the evolution SQLite database. This can show backlog/status data from the wrong workspace and can enqueue user-authorized capability evolution into a different DB than the later workspace-scoped evolution run.

## Scope

- In scope:
  - Investigate backlog/status/proposal-status/authorize-capability DB path scoping with runtime evidence.
  - Apply the smallest Rust fix needed for workspace-scoped evolution DB reads/writes.
  - Add focused regression tests for the affected command/desktop paths.
- Out of scope:
  - Broad evolution architecture refactors.
  - Changing SQLite schema or evolution policy/runtime thresholds.

## Acceptance Criteria

- [x] Runtime evidence proves the pre-fix workspace mismatch.
- [x] `--workspace`-scoped evolution backlog/status/proposal-status/authorize-capability paths use the selected workspace DB.
- [x] Regression tests cover the fixed workspace scoping behavior.
- [x] Required Rust verification commands pass.

## Risks

- Risk: mutating process environment globally to honor CLI workspace.
  - Impact: unrelated commands/tests could become order-dependent.
  - Mitigation: prefer explicit workspace-derived chat root helpers for command paths.
- Risk: over-widening the fix surface.
  - Impact: unintended command behavior drift.
  - Mitigation: limit changes to L2 evolution DB open call sites and dispatch parameter plumbing.

## Validation Plan

- Required tests:
  - Focused regression tests for workspace-scoped evolution query/enqueue paths.
  - Workspace command tests required by repo policy.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-commands`
  - `cargo test -p skilllite`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Seed two temporary workspace DBs and run the CLI before/after the fix to verify output and inserted rows come from the `--workspace` target.

## Regression Scope

- Areas likely affected:
  - `skilllite evolution backlog`
  - `skilllite evolution status`
  - `skilllite evolution proposal-status`
  - `skilllite evolution authorize-capability`
  - manual evolution trigger logging when `cmd_run --log-manual-trigger` completes
- Explicit non-goals:
  - `reset`, `disable`, `explain`, and `repair-skills` workspace semantics unless directly required by the reported L2 bug.

## Links

- Source TODO section: user report in automation prompt.
- Related PRs/issues:
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
