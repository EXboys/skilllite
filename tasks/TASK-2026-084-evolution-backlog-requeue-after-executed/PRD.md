# PRD

## Background

Evolution governance stores proposals in SQLite `evolution_backlog` with a soft-dedupe key. Soft-dedupe is meant to collapse *active* proposals of the same scope, while preserving *executed* history for acceptance linking. The schema currently enforces a global UNIQUE on `dedupe_key`, which contradicts that intent and permanently blocks re-authorization after the first successful/forced execution.

## Objective

- Same-scope / same-capability proposals can re-enter backlog after prior rows are `executed`.
- Returned and forced proposal IDs always refer to a persisted backlog row.

## Functional Requirements

- FR-1: Soft-dedupe updates an existing non-executed row for the same `dedupe_key` instead of inserting duplicates.
- FR-2: When no non-executed row exists (including when only `executed` history remains), insert a new backlog row.
- FR-3: Existing databases that still have UNIQUE(`dedupe_key`) are migrated without data loss.
- FR-4: Coordinator status updates attach to the persisted active proposal identity after upsert.

## Non-Functional Requirements

- Security: N/A (no sandbox / authz boundary change).
- Performance: one-time table rebuild on migrate; steady-state upsert remains O(1) indexed updates.
- Compatibility: preserve all existing backlog columns and historical executed rows.

## Constraints

- Technical: SQLite cannot drop a column UNIQUE in place; rebuild table.
- Timeline: minimal fix only; no planner/policy redesign.

## Success Metrics

- Metric: re-authorize after executed yields a loadable proposal_id and a queued/executing active row.
- Baseline: phantom proposal_id + `NoScope` on forced run.
- Target: persisted active row + status updates succeed.

## Rollout

- Rollout plan: merge with evolution crate tests; migration runs on next DB open.
- Rollback plan: revert commit; old UNIQUE schema would reject multi-row history (acceptable emergency only).
