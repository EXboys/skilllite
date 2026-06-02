# TASK Card

## Metadata

- Task ID: `TASK-2026-067`
- Title: Fix desktop evolution workspace scoping
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: Cursor automation
- Created: `2026-06-02`
- Target milestone: next patch release

## Problem

Recent desktop L2 CLI bridge changes can run chat, evolution status/backlog, manual evolution runs, and capability authorization with different effective `SKILLLITE_WORKSPACE` values. Because `chat_root()` uses `SKILLLITE_WORKSPACE` rather than process cwd, this can split SQLite evolution state between `~/.skilllite/chat` and `<workspace>/chat`, making manual evolution report "nothing to evolve" despite active chat state and making user-authorized proposals fail in the background. `evolution run` also writes pending evolved skills under `.skills` while the desktop pending/confirm paths use the `skills` resolver.

## Scope

- In scope:
  - Ensure desktop-spawned `skilllite` subprocesses receive an absolute `SKILLLITE_WORKSPACE` matching the resolved project root.
  - Ensure capability authorization background runs carry the same workspace scope.
  - Align `evolution run` skill root resolution with the desktop pending/confirm resolver.
  - Add regression tests that prove the environment merge and skill-root selection behavior.
- Out of scope:
  - Redesigning the assistant bridge or restoring in-process fallbacks.
  - Changing evolution scoring, LLM behavior, or sandbox policy.
  - Changing user-facing CLI flags.

## Acceptance Criteria

- [ ] Agent chat and desktop evolution CLI subprocesses use the same absolute workspace root for `SKILLLITE_WORKSPACE`.
- [ ] `evolution authorize-capability` background execution includes the workspace passed by the desktop caller.
- [ ] `evolution run` writes/reads evolved skills through `skills/` by default, falling back to `.skills/` only when appropriate.
- [ ] Regression tests cover workspace env injection and `skills` vs `.skills` selection.
- [ ] Required Rust formatting, lint, tests, and task validation pass.

## Risks

- Risk: Overriding a deliberately configured `SKILLLITE_WORKSPACE` in desktop child processes.
  - Impact: Users with custom desktop state roots could see state move to the selected project.
  - Mitigation: Desktop commands already pass an explicit workspace and `evolution run` already rebinds to it; make child env match that explicit caller contract instead of stale ambient env.

## Validation Plan

- Required tests: focused unit/regression tests plus CLI/commands test targets.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
  - `cargo test -p skilllite-commands`
  - `python3 scripts/validate_tasks.py`
- Manual checks: terminal-driven command inspection is sufficient because this fix is subprocess environment/routing behavior, not visual UI rendering.

## Regression Scope

- Areas likely affected: desktop Assistant chat subprocesses, desktop evolution CLI bridge, `skilllite evolution run`, pending skill handling.
- Explicit non-goals: runtime provisioning progress streaming and skills-list fallback policy are not changed in this task.

## Links

- Source TODO section: N/A; critical bug-finding automation.
- Related PRs/issues: recent PRs #79, #80, #81.
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`.
