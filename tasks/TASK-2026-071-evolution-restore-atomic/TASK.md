# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Make evolution snapshot restore atomic
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: `agent`
- Created: `2026-08-13`
- Target milestone: current

## Problem

`restore_extended_snapshot` deletes live `memory/evolution` and `_evolved` trees with `remove_dir_all` before copying snapshot contents back. If copy fails or the process is killed after the delete, live evolution memory and evolved skills are gone even though the snapshot still exists. Auto-rollback (`check_auto_rollback`) uses this path, so the recovery mechanism can destroy the data it is supposed to restore.

Concrete trigger: evolution has written memory shards and `_evolved` skills; metrics degrade and auto-rollback runs (or a user/CLI restore runs); `remove_dir_all` succeeds; `copy_dir_recursive` then fails (I/O error, permission, crash/OOM). Live trees are empty or partial. A later evolution snapshot can persist that empty state and prune older snapshots.

## Scope

- In scope:
  - Replace delete-then-copy for memory and skills restore with copy-to-temp then rename swap.
  - Leave destination unchanged when the snapshot copy fails.
  - Regression tests for happy-path restore, extra-file replacement, and copy-failure integrity.
- Out of scope:
  - Prompt-file restore (small files copied in place; not a tree replace).
  - Snapshot create/prune ordering.
  - Cross-process evolution file locking.
  - ChatSession workspace chat-root alignment.

## Acceptance Criteria

- [ ] Restoring an extended snapshot still restores prompts, memory shards, and evolved skills to the snapshotted content.
- [ ] Extra files present in live memory/`_evolved` but absent from the snapshot are removed by a successful restore.
- [ ] If copying the snapshot tree fails, the live destination directory is left intact.
- [ ] No leftover restore temp/backup directories remain after success or copy failure.
- [ ] `cargo test -p skilllite-evolution` and workspace `cargo test` / `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` pass.

## Risks

- Risk: `rename` of directories fails across filesystems (temp vs dest on different mounts).
  - Impact: restore returns an error; live data remains in the backup name or original dest.
  - Mitigation: copy temp next to the destination (same parent directory / same filesystem); on swap failure, rename backup back.

## Validation Plan

- Required tests:
  - Existing `extended_snapshot_restores_memory_and_skills`
  - New restore extra-file replacement test
  - New copy-failure leaves destination intact test
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-evolution`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks: N/A (unit tests cover the restore helper and public restore API)

## Regression Scope

- Areas likely affected:
  - Auto-rollback (`check_auto_rollback` → `restore_extended_snapshot`)
  - Manual/CLI snapshot restore of memory and `_evolved` skills
- Explicit non-goals:
  - Prompt snapshot file-by-file copy
  - Evolution run mutex / concurrent process locking

## Links

- Source TODO section: daily critical-bug sweep
- Related PRs/issues: none open for this restore atomicity gap
- Related docs: N/A (internal restore correctness; no command/env/docs change)
