# PRD

## Background

Run artifacts are stored at `<base>/artifacts/<run_id>/<key>`. `put` is an upsert and is exposed by `skilllite artifact-serve` and by `skilllite gateway serve` when an artifact directory is configured. The local writer tried to make that upsert atomic by writing a sibling temp file and renaming it.

## Objective

A put of one key does not delete or replace a different key. After a successful put, both keys read back the bytes that were stored.

## Functional Requirements

- FR-1: The temp path for `notes.json` is not `notes.tmp`.
- FR-2: Existing keys `notes.tmp` and `archive.tar.tmp` survive puts of `notes.json` and `archive.tar.gz`.
- FR-3: The published file is still replaced by rename, so readers do not observe a partial body of the key being written.
- FR-4: If the temp write or the rename fails, the temp file is removed.

## Non-Functional Requirements

- Security: No change to bearer checks, bind policy, or key validation.
- Performance: One extra unique name per put. No new dependency.
- Compatibility: Key syntax and the directory layout of successful keys stay the same.

## Constraints

- Technical: Rename must stay in the same directory so it remains atomic on POSIX.
- Timeline: No HTTP or CLI surface change.

## Success Metrics

- Metric: Sibling keys that previously shared a `with_extension("tmp")` path both round-trip.
- Baseline: `put("notes.json")` deletes `notes.tmp` and returns `Ok`.
- Target: Both gets return the stored bytes.

## Rollout

- Rollout plan: Ship with the local store. HTTP handlers pick it up without a route change.
- Rollback plan: Revert the writer. Callers that already lost a `.tmp` sibling cannot recover that data from this change.
