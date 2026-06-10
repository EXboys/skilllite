# PRD

## Background

Desktop L2 evolution commands are intended to operate on the workspace selected by the caller. Several JSON/desktop paths resolve the workspace for UI or `.env` data but still open the evolution DB through `skilllite_core::paths::chat_root()`, which only follows `SKILLLITE_WORKSPACE` or the global default.

## Objective

Ensure evolution L2 CLI/desktop JSON DB reads and writes use the workspace passed by `--workspace`, without broad refactors or schema changes.

## Functional Requirements

- FR-1: `evolution backlog --workspace <path>` must query `<path>/chat/feedback.sqlite`.
- FR-2: `evolution status --json --workspace <path>` must read metrics/events from `<path>/chat/feedback.sqlite`.
- FR-3: `evolution proposal-status --workspace <path>` must read the selected workspace DB.
- FR-4: `evolution authorize-capability --workspace <path>` must enqueue and audit into the selected workspace DB.
- FR-5: Existing JSON shapes and human output formats must remain unchanged.

## Non-Functional Requirements

- Security: do not relax evolution policy/runtime gating; authorization only enqueues governed proposals.
- Performance: no additional long-running DB scans beyond existing queries.
- Compatibility: callers that omit `--workspace` use the CLI default `.` and therefore resolve to the current working directory workspace.

## Constraints

- Technical: preserve crate dependency direction and avoid new dependencies.
- Timeline: high-severity correctness fix; keep changes minimal and focused.

## Success Metrics

- Metric: seeded CLI repro reads/enqueues the row in the `--workspace` DB.
- Baseline: pre-fix command uses the env/default DB instead of the CLI workspace.
- Target: post-fix command uses the CLI workspace DB and regression tests pass.

## Rollout

- Rollout plan: ship as a narrow command-layer bug fix with regression tests.
- Rollback plan: revert the command-layer DB path plumbing if regressions appear.
