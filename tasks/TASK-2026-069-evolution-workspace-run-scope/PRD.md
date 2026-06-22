# PRD

## Background

`TASK-2026-068` fixed several L2 evolution database readers/writers to honor the CLI `--workspace` argument. A follow-up inspection found remaining chat/run paths that still rely on cwd/env defaults or a hardcoded `.skills` output root. These paths affect desktop chat decisions, user-authorized evolution, and automatic desktop/agent evolution, where a silent workspace split makes generated pending skills appear lost.

## Objective

Evolution execution and desktop chat decision recording must use the same workspace and skill directory contract as the desktop status/backlog/pending UI. A chat turn, user-triggered run, or automatic run for workspace `W` must not write proposals, decisions, or generated skills under a different workspace root.

## Functional Requirements

- FR-1: `skilllite evolution run --workspace W` resolves skill output via the shared `skills` with `.skills` legacy fallback helper.
- FR-2: Desktop `agent-rpc` child processes receive `SKILLLITE_WORKSPACE=W` after UI workspace resolution.
- FR-3: Agent in-process A9 evolution resolves skill output via the shared fallback helper.
- FR-4: Desktop background follow-up runs after authorization include `--workspace W`.
- FR-5: Life Pulse growth runs include `--workspace W`.
- FR-6: Tests must prove argument/root/env selection without requiring a live LLM provider.

## Non-Functional Requirements

- Security: Preserve existing path validation and gatekeeper boundaries; do not widen allowed write locations beyond the effective workspace skill root.
- Performance: Keep changes to constant-time path/argument construction.
- Compatibility: Continue supporting legacy workspaces that only contain `.skills`.

## Constraints

- Technical: Avoid broad refactors in Tauri command wiring and avoid requiring an LLM key in regression tests.
- Timeline: N/A for autonomous task execution; scope is constrained to known run paths and focused tests.

## Success Metrics

- Metric: Workspace run paths that explicitly carry `--workspace`.
- Baseline: Manual trigger carries `--workspace`; authorize follow-up and Life Pulse growth do not.
- Target: All desktop `evolution run` subprocess paths carry `--workspace`.
- Metric: Desktop chat/evolution DB agreement.
- Baseline: `agent-rpc` can write chat data under `~/.skilllite/chat` while L2 reads `<workspace>/chat`.
- Target: Desktop `agent-rpc` receives `SKILLLITE_WORKSPACE=<workspace>`.
- Metric: Skill output root agreement.
- Baseline: `cmd_run` writes `.skills`; desktop pending reads effective `skills`/`.skills`.
- Target: `cmd_run` and desktop pending use the same effective root.

## Rollout

- Rollout plan: Ship as a narrow bug-fix PR with focused tests and no schema migration.
- Rollback plan: Revert the commit if regressions appear; no persisted data migration is introduced.
