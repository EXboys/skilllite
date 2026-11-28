# PRD

## Background

Agent deliverable writes and the local preview server are supposed to stay inside a chosen directory. Both checks were lexical. On 2026-11-28, `origin/main` was still `f07126c`. A symlink planted in `output/` or in a preview directory was enough to write or read a file outside that directory.

## Objective

Fail closed when a `write_output` path or a preview request follows a symlink outside its root, without blocking ordinary files inside that root.

## Functional Requirements

- FR-1: `write_output` must reject a symlink file whose canonical target is outside the output root, for both overwrite and append, and must leave the target bytes unchanged.
- FR-2: `write_output` must reject a new file whose nearest existing ancestor is a symlink directory outside the output root, and must not create that file.
- FR-3: When `SKILLLITE_OUTPUT_DIR` is unset, `<workspace>/output` must not be a symlink whose canonical path leaves the workspace.
- FR-4: Preview file resolution must reject the same class of symlink, and must still reject encoded `..` traversal.
- FR-5: A non-symlink Unicode filename must still read and write.

## Non-Functional Requirements

- Security: Default behavior becomes stricter. No new approval bypass and no new network permission.
- Performance: One `canonicalize` of the root and one of the nearest existing ancestor per checked path.
- Compatibility: An explicit output directory outside the workspace remains the user's configured root. Symlinks that stay inside a root remain usable.

## Constraints

- Technical: Do not reopen the workspace resolver work in open PR #132. Task id `084` avoids unmerged task numbers `075`–`083`.
- Timeline: Single bugfix.

## Success Metrics

- Metric: Regression tests fail before the check and pass after it.
- Baseline: `write_output` returned `Successfully wrote 5 bytes to .../output/leak`.
- Target: The same call returns an error containing `symlink`, and the outside file still contains `保留 中文`.

## Rollout

- Rollout plan: Ship with the agent builtin tools. No env or command change.
- Rollback plan: Revert the containment checks in `output.rs`, `preview.rs`, and `helpers.rs`.
