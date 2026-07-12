# PRD

## Background

Recent workspace scoping fixes made desktop chat and evolution write/read paths use the active project workspace. Life Pulse rhythm still checks scheduled job due-ness in-process against the active workspace, then starts a child CLI process without passing that workspace. The CLI already supports `schedule tick --workspace`, and the desktop heartbeat should use that interface so check and execution paths share the same root.

## Objective

- Scheduled agent jobs detected as due in the active desktop workspace are executed against that same workspace.
- No new scheduling semantics are introduced; this is a binding fix for an existing CLI option.

## Functional Requirements

- FR-1: Life Pulse rhythm must pass `--workspace <active workspace>` to `skilllite schedule tick`.
- FR-2: The argument list must preserve workspace paths containing spaces as one argument.

## Non-Functional Requirements

- Security: Do not relax schedule execution gating; non-dry-run execution still requires `SKILLLITE_SCHEDULE_ENABLED=1`.
- Performance: No extra polling or filesystem work beyond the existing due check.
- Compatibility: Preserve existing dotenv/settings merge and child process environment behavior.

## Constraints

- Technical: Keep the fix local to the Tauri Life Pulse subprocess helper.
- Timeline: N/A for autonomous execution; scope is a minimal bug fix.

## Success Metrics

- Metric: Argument builder coverage for rhythm workspace propagation.
- Baseline: Rhythm subprocess args are `["schedule", "tick"]`.
- Target: Rhythm subprocess args are `["schedule", "tick", "--workspace", workspace]`.

## Rollout

- Rollout plan: Ship as a normal patch to the desktop assistant crate.
- Rollback plan: Revert the Life Pulse rhythm argument change if it causes unexpected desktop schedule behavior.
