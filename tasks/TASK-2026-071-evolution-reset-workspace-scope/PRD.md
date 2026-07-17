# PRD

## Background

Recent evolution workspace fixes made status, backlog, pending-skill, and run
operations use explicit workspace roots. The destructive reset command was
left on legacy path helpers, so its chat and skill targets can diverge from
the workspace used to create the evolution data.

## Objective

Reset all evolution artifacts belonging to exactly one requested workspace,
including modern and legacy skill directory layouts, without touching another
workspace selected through the process environment.

## Functional Requirements

- FR-1: `evolution reset` accepts `--workspace/-w`, defaulting to `.`.
- FR-2: Chat prompts, database logs, JSONL logs, and snapshots are resolved
  under `<workspace>/chat`.
- FR-3: Evolved skills are removed from the workspace's effective skills
  directory, preferring `skills/` with `.skills/` fallback.
- FR-4: Without `--force`, reset remains non-mutating.

## Non-Functional Requirements

- Security: Destructive operations must not escape the resolved workspace.
- Performance: No additional directory traversal beyond existing reset work.
- Compatibility: Preserve the legacy `.skills/` fallback and existing output.

## Constraints

- Technical: Reuse existing workspace and skill-directory resolvers.
- Timeline: N/A; completion is gated by verification evidence.

## Success Metrics

- Metric: Workspace isolation in the reset integration test.
- Baseline: Reset uses global chat state and always targets `.skills/`.
- Target: All reset mutations stay inside the explicit workspace and remove
  the effective evolved-skills directory.

## Rollout

- Rollout plan: Ship as a backward-compatible CLI flag addition with a safer
  project-local default.
- Rollback plan: Revert the focused CLI, dispatch, command, test, and docs
  changes together.
