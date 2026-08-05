# TASK-2026-081: Fix artifact temp write collision and proposal ID collision

## Summary

Two independent critical correctness bugs on `main` (`12010e8`):

1. `LocalDirArtifactStore` atomic writes derive the temp path via `Path::with_extension("tmp")`, so distinct keys that share a stem (e.g. `report.json` / `report.csv`) collide on the same temp file and can corrupt or lose artifact bytes under concurrent PUTs. Keys ending in `.tmp` also skip the temp+rename pattern entirely.
2. Evolution `proposal_id` values are generated from millisecond timestamps only. When passive and active proposals are built in the same millisecond, `INSERT OR IGNORE` on the unique `proposal_id` silently drops one backlog row while in-memory execution can still proceed under the duplicated ID.

## Scope

- Fix `crates/skilllite-artifact/src/local_dir.rs` temp naming for atomic writes.
- Fix proposal ID generation in `crates/skilllite-evolution/src/scope.rs` (both `build_proposal` and capability-authorization construction).
- Add regression tests for both behaviors.
- No broad refactors.

## Non-goals

- Merging or rebasing older open critical-fix PRs (#89, #112–#132).
- Changing artifact HTTP API surface or evolution coordinator policy.
- Fixing `skilllite_fs::atomic_write` in this PR (same `with_extension("tmp")` pattern exists but callers typically target known unique filenames; tracked as follow-up if needed).

## Acceptance Criteria

- [ ] Concurrent / sequential PUTs for `report.json` and `report.csv` under the same run keep independent correct payloads.
- [ ] A key ending in `.tmp` still uses a distinct temp path (atomic rename preserved).
- [ ] Two proposals built in the same process within the same millisecond receive distinct `proposal_id` values.
- [ ] Both proposal-ID construction sites use the unique generator.
- [ ] `cargo test -p skilllite-artifact`, focused evolution unit tests, `cargo fmt --check`, and clippy for touched crates pass.
- [ ] Task artifacts and `tasks/board.md` updated.

## Risks

- Temp-file naming changes may leave orphaned `*.tmp-*` files if a process crashes mid-write (same class of risk as before; acceptable).
- Proposal ID format gains a uniqueness suffix; consumers that assumed a pure timestamp suffix must tolerate the longer ID (IDs are opaque strings today).

## Validation Plan

- Unit tests in `skilllite-artifact` for stem-collision and `.tmp` key paths.
- Unit test(s) in `skilllite-evolution` proving distinct IDs across rapid `build_proposal` calls and backlog upsert of both rows.
- Run required Cargo checks; record command output in `STATUS.md`.

## Regression Scope

- Artifact local store put/get.
- Evolution backlog upsert / coordinator selection by `proposal_id`.
