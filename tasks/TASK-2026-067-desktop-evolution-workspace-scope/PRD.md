# PRD

## Background

The desktop Assistant moved multiple evolution and runtime surfaces to CLI subprocesses. The CLI commands accept `--workspace`, but shared state roots still depend on `SKILLLITE_WORKSPACE`. If the desktop process only sets cwd or passes `--workspace` to some commands, the chat session, evolution backlog, manual run, and background authorization can observe different SQLite databases.

## Objective

All desktop-triggered chat/evolution subprocesses must treat the selected project root as the single workspace state root. Evolved skill creation and desktop pending/confirm operations must resolve the same skill root.

## Functional Requirements

- FR-1: Desktop child process environment must include absolute `SKILLLITE_WORKSPACE=<resolved project root>` after dotenv/UI overrides are merged.
- FR-2: Capability authorization must run the background forced evolution against the same workspace as the enqueue command.
- FR-3: `evolution run` must select `skills/` by default and use `.skills/` only as legacy fallback when `skills/` is absent and `.skills/` exists.

## Non-Functional Requirements

- Security: Do not loosen sandbox or gatekeeper path checks.
- Performance: No additional long-running work in the desktop command path.
- Compatibility: Preserve existing CLI flags and keep `.skills/` fallback for legacy workspaces.

## Constraints

- Technical: Keep changes within existing bridge/command layers; do not introduce new crate dependencies.
- Timeline: N/A for autonomous execution.

## Success Metrics

- Metric: Workspace state root consistency across desktop chat and evolution subprocesses.
- Baseline: Chat can write `~/.skilllite/chat` while `evolution run --workspace` reads `<workspace>/chat`.
- Target: Both paths receive the same absolute `SKILLLITE_WORKSPACE`.

## Rollout

- Rollout plan: Ship as a patch-level bug fix with regression tests.
- Rollback plan: Revert the bridge env injection and skill-root resolver change if unexpected compatibility regressions appear.
