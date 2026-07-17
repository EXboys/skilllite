# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Scope evolution reset to the requested workspace
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-17`
- Target milestone:

## Problem

`skilllite evolution reset --force` resets `paths::chat_root()` but removes
evolved skills only from the legacy `.skills/` directory. When a user targets
a modern project workspace, the command can delete global/default chat
evolution data while leaving the requested workspace's `skills/_evolved`
artifacts active.

## Scope

- In scope:
  - Add explicit workspace selection to `evolution reset`.
  - Resolve both chat state and evolved skills from that same workspace.
  - Preserve the existing `.skills/` fallback when `skills/` is absent.
  - Add an integration regression test and synchronized EN/ZH command docs.
- Out of scope:
  - Workspace semantics for `disable`, `explain`, or `repair-skills`.
  - Evolution engine or desktop refactors.

## Acceptance Criteria

- [x] `evolution reset --force --workspace <target>` modifies only the target
  workspace's chat evolution state.
- [x] Reset removes both `skills/_evolved` and `.skills/_evolved` when present,
  preventing either supported discovery layout from retaining evolved skills.
- [x] A regression test proves an unrelated env workspace remains untouched.
- [x] EN/ZH command references document the workspace-scoped reset.

## Risks

- Risk: Changing the no-flag reset default from global/env-derived state to the
  current workspace may surprise scripts that relied on implicit global data.
  - Impact: A script may reset a local project instead of global state.
  - Mitigation: Match the established default `--workspace .` semantics used by
    other evolution commands and document the behavior.

## Validation Plan

- Required tests:
  - CLI integration test covering target and unrelated workspaces.
  - Full Rust formatting, lint, and test gates.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Inspect `evolution reset --help` for the workspace flag.

## Regression Scope

- Areas likely affected:
  - Evolution CLI parsing and dispatch.
  - Reset path resolution and destructive filesystem/database operations.
- Explicit non-goals:
  - Non-reset evolution command cleanup.
  - Changes to evolution data formats or schemas.

## Links

- Source TODO section: N/A (critical bug automation finding)
- Related PRs/issues: PR #95, PR #101
- Related docs: `README.md`, `docs/zh/README.md`
