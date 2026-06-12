# PRD

## Background

The daily critical-bug investigation found concrete failure modes in recent
evolution behavior. The affected paths are visible to desktop and CLI users:
status inspection can crash on Unicode log content, and background evolution can
write generated skills outside the root used by pending-skill UI actions.

## Objective

Evolution status and run paths should be consistent, panic-free, and scoped to
the workspace selected by the user.

## Functional Requirements

- FR-1: Human evolution status output must truncate event reasons without slicing
  through UTF-8 code points.
- FR-2: Evolution run skill output must resolve the same `skills/` or legacy
  `.skills/` root as desktop pending-skill operations.
- FR-3: Life Pulse growth runs must pass the selected workspace to the child
  `skilllite evolution run` command.

## Non-Functional Requirements

- Security: No sandbox or permission policy changes.
- Performance: Path and string handling changes must be constant or linear in
  small display strings only.
- Compatibility: Preserve legacy `.skills` fallback when `skills/` does not
  exist.

## Constraints

- Technical: Keep crate dependency direction unchanged and reuse existing core
  skill discovery helpers.
- Timeline: N/A.

## Success Metrics

- Metric: Unicode evolution status preview does not panic.
- Baseline: Byte slicing `reason[..47]` panics when byte 47 is not a char boundary.
- Target: Regression test passes with multibyte input.
- Metric: Evolved skills root matches desktop pending-skill root.
- Baseline: `evolution run` uses `.skills` while desktop uses `skills/` with
  legacy fallback.
- Target: Regression tests pass for primary `skills/` and legacy `.skills`.

## Rollout

- Rollout plan: Ship as a small bug-fix PR.
- Rollback plan: Revert this task's commit if unexpected path behavior appears.
