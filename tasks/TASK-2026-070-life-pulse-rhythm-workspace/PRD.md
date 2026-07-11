# PRD

## Background

Recent workspace-scoping fixes focused on desktop subprocesses that need to read
and write project-local state. Life Pulse has two background subprocess paths:
growth and rhythm. Growth now forwards the workspace explicitly, but rhythm still
starts `schedule tick` without `--workspace`.

## Objective

When Life Pulse detects due scheduled jobs in the active workspace, the spawned
rhythm subprocess must execute `schedule tick` against that same workspace.

## Functional Requirements

- FR-1: Build rhythm subprocess arguments as `schedule tick --workspace <workspace>`.
- FR-2: Pass the active Life Pulse workspace into rhythm spawning at the call site.
- FR-3: Preserve existing environment propagation for dotenv and UI LLM overrides.

## Non-Functional Requirements

- Security: No new permissions or sandbox bypasses.
- Performance: No additional polling or filesystem scans.
- Compatibility: CLI defaults remain unchanged for users invoking `schedule tick` directly.

## Constraints

- Technical: Keep the fix local to the desktop Life Pulse bridge.
- Timeline: N/A for autonomous execution; complete once tests and task gates pass.

## Success Metrics

- Metric: Unit coverage for rhythm argument construction.
- Baseline: Rhythm args omit `--workspace`.
- Target: Rhythm args include the active workspace exactly once.

## Rollout

- Rollout plan: Merge as a desktop subprocess correctness fix.
- Rollback plan: Revert the Life Pulse argument helper and call-site change.
