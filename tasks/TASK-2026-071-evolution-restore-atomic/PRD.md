# PRD

## Background

Evolution rollback is the recovery path when a run degrades metrics or a user restores a snapshot. The extended restore currently deletes live `memory/evolution` and `skills/_evolved` before copying from `prompts/_versions/<txn>`. That ordering turns a failed restore into data loss of the current trees.

## Objective

- Restore of memory and evolved-skill trees must not destroy live data unless the replacement tree is already fully copied and swapped into place.
- Successful restore must still replace the live tree (including deleting files that were added after the snapshot).

## Functional Requirements

- FR-1: `restore_extended_snapshot` copies snapshot memory/skills into a sibling temp directory, then swaps that directory into the live path.
- FR-2: If the snapshot copy fails, the live destination is unchanged.
- FR-3: If the final rename fails after the live directory was moved aside, restore attempts to move the backup back.
- FR-4: Successful restore removes files that exist live but not in the snapshot (same observable result as delete-then-copy).

## Non-Functional Requirements

- Security: no sandbox/policy change; restore still only writes under the provided `chat_root` / `skills_root`.
- Performance: one extra directory rename; copy cost unchanged.
- Compatibility: snapshot layout and txn ids unchanged; callers of `restore_extended_snapshot` unchanged.

## Constraints

- Technical: keep the fix inside `skilllite-evolution` snapshot restore; no crate boundary changes.
- Timeline: N/A (autonomous bugfix).

## Success Metrics

- Metric: copy-failure test leaves live content intact.
- Baseline: live tree deleted before copy; failure loses data.
- Target: live tree intact on copy failure; restored content matches snapshot on success.

## Rollout

- Rollout plan: merge the restore helper change; no migration.
- Rollback plan: revert the commit; previous delete-then-copy behavior returns.
