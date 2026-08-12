# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Fix swarm workspace retarget containment bypass
- Status: `in_progress`
- Priority: `P0`
- Owner: `cursor-agent`
- Contributors:
- Created: `2026-08-12`
- Target milestone:

## Problem

`delegate_to_swarm` copied an LLM-supplied `workspace` into `NodeTask.context`, and
`AgentTaskExecutor` / `run_single_task` used that value as `AgentConfig.workspace`.
A model (or HTTP `/task` client) could retarget local swarm execution to an arbitrary
filesystem root, defeating `write_file` workspace containment and enabling silent
writes under `SilentEventSink`.

## Scope

- In scope:
  - Force `delegate_to_swarm` to embed the calling agent workspace only.
  - Make `AgentTaskExecutor` execute under the node workspace (skills-dir parent / cwd).
  - Regression tests for both clamp points.
- Out of scope:
  - SilentEventSink ConfirmRequired policy (#138).
  - Memory-flush tool allowlist (#139).
  - ChatSession / memory chat-root alignment.
  - Evolution restore atomicity.

## Acceptance Criteria

- [x] LLM-provided `workspace` overrides cannot change delegated `NodeTask.context.workspace`.
- [x] Local swarm execution ignores client-supplied `context.workspace` filesystem roots.
- [x] Regression tests cover override-ignore and skills-dir parent resolution.
- [x] Required validation commands pass for the touched crates.

## Risks

- Risk: Peers that previously relied on client absolute workspace paths for local FS work.
  - Impact: Tasks run under the node's project root instead of a foreign absolute path.
  - Mitigation: Intended for P2P (foreign absolute paths are meaningless on peers); node
    workspace from `--skills-dir` / cwd is the correct execution root.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent effective_delegate_workspace`
  - `cargo test -p skilllite resolve_local_workspace` (with agent+swarm features)
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings` (scoped if pre-existing blockers)
- Commands to run: see STATUS.md evidence.
- Manual checks: N/A

## Regression Scope

- Areas likely affected:
  - `delegate_to_swarm` tool schema/behavior
  - `skilllite swarm` local task execution workspace
- Explicit non-goals:
  - Swarm auth / routing algorithm changes
  - Desktop Life Pulse / chat-root splits

## Links

- Source TODO section: critical-bug-investigation automation (2026-08-12)
- Related PRs/issues: open security backlog #138/#139 (amplifiers, not root cause)
- Related docs: `docs/en/ARCHITECTURE.md` swarm section (no new env/command; schema note only)
