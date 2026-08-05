# TASK Card

## Metadata

- Task ID: `TASK-2026-081`
- Title: Fix artifact temp write collision and proposal ID collision
- Status: `in_review`
- Priority: `P0`
- Owner: `cursor-automation`
- Contributors:
- Created: `2026-08-05`
- Target milestone:

## Problem

Two independent critical correctness bugs on `main` (`12010e8`):

1. `LocalDirArtifactStore` atomic writes derive the temp path via `Path::with_extension("tmp")`, so distinct keys that share a stem (e.g. `report.json` / `report.csv`) collide on the same temp file and can corrupt or lose artifact bytes under concurrent PUTs. Keys ending in `.tmp` also skip the temp+rename pattern entirely.
2. Evolution `proposal_id` values are generated from millisecond timestamps only. When passive and active proposals are built in the same millisecond, `INSERT OR IGNORE` on the unique `proposal_id` silently drops one backlog row while in-memory execution can still proceed under the duplicated ID.

## Scope

- In scope:
  - Fix `crates/skilllite-artifact/src/local_dir.rs` temp naming for atomic writes.
  - Fix proposal ID generation in `crates/skilllite-evolution/src/scope.rs` (both `build_proposal` and capability-authorization construction).
  - Add regression tests for both behaviors.
- Out of scope:
  - Merging or rebasing older open critical-fix PRs (#89, #112–#132).
  - Changing artifact HTTP API surface or evolution coordinator policy.
  - Fixing `skilllite_fs::atomic_write` in this PR (same `with_extension("tmp")` pattern exists but callers typically target known unique filenames).

## Acceptance Criteria

- [x] Concurrent / sequential PUTs for `report.json` and `report.csv` under the same run keep independent correct payloads.
- [x] A key ending in `.tmp` still uses a distinct temp path (atomic rename preserved).
- [x] Two proposals built in the same process within the same millisecond receive distinct `proposal_id` values.
- [x] Both proposal-ID construction sites use the unique generator.
- [x] `cargo test -p skilllite-artifact`, focused evolution unit tests, `cargo fmt`, and clippy for touched crates pass.
- [x] Task artifacts and `tasks/board.md` updated.

## Risks

- Risk: Temp-file naming changes may leave orphaned `*.tmp-*` files if a process crashes mid-write.
  - Impact: Disk clutter only; same class of risk as before.
  - Mitigation: Acceptable; staging names remain under the artifact directory.
- Risk: Proposal ID format gains a uniqueness suffix.
  - Impact: Consumers that assumed a pure timestamp suffix must tolerate a longer ID.
  - Mitigation: IDs are opaque strings today; no schema migration required.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-artifact --lib`
  - `cargo test -p skilllite-evolution --lib proposal_ids_remain_unique`
  - `cargo test -p skilllite-evolution --lib coordinator_persists_both_proposals`
  - `cargo clippy -p skilllite-artifact -p skilllite-evolution --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Commands to run: see STATUS.md evidence.
- Manual checks: N/A.

## Regression Scope

- Artifact local store put/get.
- Evolution backlog upsert / coordinator selection by `proposal_id`.

## Notes

Related open PRs cover other critical paths but not these two bugs (#123 touches artifact validation only; #117/#90 cover authorized proposal routing).
