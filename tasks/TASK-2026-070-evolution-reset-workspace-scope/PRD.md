# PRD

## Background

`TASK-2026-068` and `TASK-2026-069` fixed workspace splits in evolution status,
backlog, authorization, run, and pending skill flows. Follow-up review found the
remaining destructive/manual prompt commands still rely on global path resolution.
This is dangerous for `reset --force`: with no `SKILLLITE_WORKSPACE`, prompt state
resolves under `~/.skilllite/chat` while evolved skill deletion resolves from the
current directory, so one command can mutate two different roots.

## Objective

All manual evolution prompt-management commands should accept the same
workspace target as the rest of the evolution CLI. Destructive reset must not mix
home/global chat state with project-local skills.

## Functional Requirements

- FR-1: `evolution reset` accepts `--workspace/-w` with default `.`.
- FR-2: `evolution reset --force --workspace W` uses `W/chat` for prompt/log/database state.
- FR-3: `evolution reset --force --workspace W` deletes evolved skills under the effective `skills` or legacy `.skills` root for `W`.
- FR-4: `evolution disable` and `evolution explain` accept `--workspace/-w` and read/write `W/chat/prompts/rules.json`.
- FR-5: Existing human output and JSON surfaces remain unchanged apart from help text.

## Non-Functional Requirements

- Security: Do not add new delete targets beyond the selected workspace's chat and evolved skill roots.
- Performance: No material runtime change; path resolution remains local filesystem work.
- Compatibility: Preserve default `.` semantics used by existing workspace-scoped evolution commands.

## Constraints

- Technical: Keep changes inside entry/commands layers; do not alter lower-layer evolution storage schemas.
- Timeline: N/A for autonomous execution; scope is limited to CLI plumbing, tests, and docs.

## Success Metrics

- Metric: Workspace-scoped reset under env mismatch.
- Baseline: Reset uses `paths::chat_root()` and `resolve_skills_root(None)`, which can target different roots.
- Target: Reset uses the explicit/default workspace root for both chat and skills paths.

## Rollout

- Rollout plan: Ship as a CLI bug fix with regression coverage and docs updates.
- Rollback plan: Revert the CLI argument and command plumbing commit if unexpected compatibility issues appear.
