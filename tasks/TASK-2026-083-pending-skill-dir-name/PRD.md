# PRD

## Background

Pending evolved skills live in `<skills>/_evolved/_pending/<name>`. Confirm moves that directory to `<skills>/_evolved/<name>`. Reject deletes it. The CLI and the desktop bridge pass the skill name through unchanged.

## Objective

Confirm and reject operate only on one direct child of `_evolved/_pending`.

## Functional Requirements

- FR-1: A name that is one normal path component is unchanged: confirm renames that child, reject deletes that child.
- FR-2: An absolute name is refused before any remove or rename.
- FR-3: `.`, `..`, and any name with more than one path component are refused before any remove or rename.
- FR-4: A non-ASCII single component such as `日程` remains valid.

## Non-Functional Requirements

- Security: a reject or confirm command must not delete or rename a directory outside `_pending`.
- Performance: one path-component check per call.
- Compatibility: existing single-component pending names, including non-ASCII names, still confirm and reject.

## Constraints

- Technical: validation happens in `skilllite-evolution` so both the CLI and the desktop bridge are covered.
- Timeline: bugfix only. No migration of existing pending directories.

## Success Metrics

- Metric: unit tests keep an outside directory after reject and confirm are given an escaping name.
- Baseline: before the fix, an absolute name is joined as that absolute path and `remove_dir_all` deletes it.
- Target: the call returns a validation error and the outside directory still contains its marker file.

## Rollout

- Rollout plan: ship with the next `skilllite` binary. No data migration.
- Rollback plan: revert the commit. Valid single-component names do not depend on the new check.
