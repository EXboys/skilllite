# TASK Card

## Metadata

- Task ID: TASK-2026-084-evolution-backlog-requeue-after-executed
- Title: Allow evolution backlog re-queue after executed
- Status: `in_review`
- Priority: `P0`
- Owner: automation
- Contributors:
- Created: 2026-08-08
- Target milestone:

## Problem

`evolution_backlog.dedupe_key` is globally UNIQUE while `upsert_backlog_proposal` only updates rows with `status != 'executed'`. After a proposal is marked `executed`, a later same-scope / same-capability authorization cannot insert a new backlog row. `enqueue_user_capability_evolution` then returns a phantom `proposal_id`, and forced `evolution run` resolves to `NoScope` — authorized re-evolution silently fails.

## Scope

- In scope:
  - Drop global UNIQUE on `evolution_backlog.dedupe_key` (new schema + existing DB migration).
  - Make upsert update active rows or insert a new row when only executed history exists.
  - Resolve persisted proposal IDs before coordinator status updates.
  - Regression tests for re-queue after executed and soft-dedupe of non-executed rows.
- Out of scope:
  - Broader coordinator policy changes.
  - Concurrent `sessions.json` last-writer-wins (separate follow-up).
  - Proposal ID UUID collision fix already tracked in open PR #133.

## Acceptance Criteria

- [x] After an `executed` row exists for a dedupe key, a new enqueue/upsert creates a fresh non-executed backlog row.
- [x] `enqueue_user_capability_evolution` returns a proposal_id that exists in the DB after re-auth.
- [x] Soft-dedupe still collapses multiple non-executed inserts for the same key to one active row.
- [x] Existing DBs created with UNIQUE on `dedupe_key` migrate successfully.
- [x] Focused evolution crate tests pass.

## Risks

- Risk: Migration rebuilds `evolution_backlog` on existing user DBs.
  - Impact: Temporary lock / failure if table shape unexpected.
  - Mitigation: Detect UNIQUE via `sqlite_master.sql`; copy all columns; recreate indexes; cover with migration test.
- Risk: Multiple historical rows per dedupe key change query assumptions.
  - Impact: Readers that assume one row per key could mis-count.
  - Mitigation: Keep `status != 'executed'` filters on active-path loaders; preserve UNIQUE on `proposal_id`.

## Validation Plan

- Required tests:
  - Re-queue after executed for user capability enqueue.
  - Soft-dedupe of consecutive non-executed enqueues.
  - Schema migration from UNIQUE dedupe_key.
  - Coordinator status attach after re-queue (optional focused unit).
- Commands to run:
  - `cargo test -p skilllite-evolution --lib`
  - `cargo clippy -p skilllite-evolution --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - N/A (unit coverage of the failure path).

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-evolution` backlog upsert / authorize / coordinator.
- Explicit non-goals:
  - Desktop UI changes; CLI flag surface; docs site.

## Links

- Source TODO section: critical bug automation sweep 2026-08-08
- Related PRs/issues: follow-up to #133 (proposal_id collisions); distinct from #135
- Related docs: N/A
