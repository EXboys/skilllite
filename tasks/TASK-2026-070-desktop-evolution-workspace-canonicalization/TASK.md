# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Canonicalize desktop evolution workspace roots
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-04`
- Target milestone:

## Problem

Desktop chat canonicalizes the configured workspace with `find_project_root` before spawning
`agent-rpc`, but the desktop evolution UI passes the raw workspace string to CLI
`--workspace` arguments. In monorepos or nested app folders, chat/A9 can write decisions and
skills under the project root while evolution status, backlog, pending, authorize, and run
commands read or write under the nested path.

## Scope

- In scope:
  - Canonicalize desktop evolution CLI `--workspace` arguments to the same project root used by chat.
  - Cover status/backlog/proposal/pending/confirm/reject/authorize/manual/life-pulse evolution runs.
  - Add regression tests for nested workspaces with parent skill roots.
- Out of scope:
  - CLI-only `evolution reset` / `disable` / `explain` workspace flag design.
  - Broad refactors of workspace discovery or `.env` loading semantics.

## Acceptance Criteria

- [ ] Desktop evolution UI commands use the canonical project root for CLI `--workspace`.
- [ ] Raw workspace remains available for `.env` lookup and process current-directory resolution.
- [ ] Regression tests prove nested workspace paths are canonicalized before CLI arguments are built.
- [ ] Required Rust formatting, linting, tests, and task validation pass or any environment blockers are recorded.

## Risks

- Risk: Changing workspace argument construction could break explicit workspace selection.
  - Impact: Users might see evolution data from a parent project when they intentionally selected a nested workspace.
  - Mitigation: Match existing chat/A9 behavior, which already treats the parent skill root as the project root.

## Validation Plan

- Required tests:
  - Focused tests for assistant evolution UI argument construction.
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Commands to run:
  - Run focused tests first, then required workspace checks.
- Manual checks:
  - Re-read modified files and task board after edits.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant` desktop evolution UI subprocess arguments.
  - Desktop life pulse growth subprocess arguments.
- Explicit non-goals:
  - Sandbox behavior.
  - Python SDK behavior.
  - CLI workspace flag surface beyond desktop bridge usage.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
