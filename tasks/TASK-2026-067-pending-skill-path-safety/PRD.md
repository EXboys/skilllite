# PRD

## Background

Recent evolution L2 CLI work exposed pending skill confirmation and rejection
through desktop-facing commands. Those commands pass `skill_name` through to
filesystem operations that currently accept path separators and absolute paths.
This creates an arbitrary directory delete/move risk in a trusted local CLI and
desktop process.

## Objective

Constrain pending skill operations so the identifier can only name one pending
skill directory under `_evolved/_pending`, and preserve the existing valid
confirm/reject workflow.

## Functional Requirements

- FR-1: Reject pending skill identifiers containing path separators, absolute
  path semantics, `.` or `..` segments, or empty/whitespace-only names.
- FR-2: Apply the same validation to pending skill read, confirm, and reject
  paths.
- FR-3: Keep successful confirm/reject behavior unchanged for safe skill names.

## Non-Functional Requirements

- Security: pending skill operations must remain confined to the pending skill
  directory and must not delete or move arbitrary filesystem paths.
- Performance: validation should be constant-time over a short identifier and
  add no meaningful overhead.
- Compatibility: generated/listed pending skill names that are normal directory
  names remain accepted.

## Constraints

- Technical: avoid new dependencies and keep the fix in shared lower-level code
  where CLI and desktop callers both benefit.
- Timeline: immediate P0 bug fix; avoid broad refactors.

## Success Metrics

- Metric: traversal regression tests.
- Baseline: unsafe names can be joined outside `_pending`.
- Target: unsafe names return validation errors and out-of-scope directories are
  left intact.

## Rollout

- Rollout plan: ship as a patch release / normal merge once tests pass.
- Rollback plan: revert the small validation change if it blocks legitimate
  generated skill names; no data migration is involved.
