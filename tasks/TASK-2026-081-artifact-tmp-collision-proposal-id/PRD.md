# PRD — Artifact temp collision and proposal ID uniqueness

## Problem

### Artifact store

`LocalDirArtifactStore::put` writes through `atomic_write_bytes`, which builds a temp path with `path.with_extension("tmp")`. That replaces the final extension, so:

- `artifacts/<run>/report.json` and `artifacts/<run>/report.csv` both stage via `artifacts/<run>/report.tmp`
- Concurrent (or overlapping) writes can rename the wrong payload into a destination, or leave one key with the other's bytes
- A key of `report.tmp` stages and finalizes at the same path, defeating atomicity

### Evolution proposals

`build_proposal` (and the capability-authorization path) mint IDs as `proposal_<YYYYMMDD_HHMMSS.mmm>`. `build_evolution_proposals` can emit passive and active proposals back-to-back. When both land in the same millisecond:

- Both share one `proposal_id` but different `dedupe_key`
- `upsert_backlog_proposal` uses `INSERT OR IGNORE` on unique `proposal_id`, so the second insert is dropped
- Status updates keyed by `proposal_id` can attach to the wrong persisted row relative to the in-memory selected proposal

## Goals

- Preserve atomic rename semantics without stem collisions.
- Guarantee unique evolution proposal IDs under rapid successive generation.
- Keep fixes minimal and regression-tested.

## Non-goals

- Redesigning the artifact store layout or HTTP API.
- Changing coordinator selection / ROI policy.
- Addressing other known open-PR issues (path traversal, workspace scoping, etc.).

## Requirements

1. Temp paths for artifact writes MUST be unique per destination key (and preferably per attempt).
2. Proposal IDs MUST remain unique even when generated within the same millisecond.
3. Existing opaque-string consumers of `proposal_id` MUST continue to work without schema migrations.
