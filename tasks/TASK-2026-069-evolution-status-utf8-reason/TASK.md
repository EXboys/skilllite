# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Fix UTF-8 crash in evolution status reasons
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors: Cursor automation
- Created: `2026-06-11`
- Target milestone: critical bug investigation

## Problem

`skilllite evolution status` human output shortens recent event reasons with byte slicing. A long non-ASCII reason stored in `evolution_log` can panic when byte index 47 lands inside a UTF-8 character, making the status command unusable for affected workspaces.

## Scope

- In scope: replace the unsafe human status reason preview with UTF-8-safe truncation and add a regression test that exercises the status command with non-ASCII event text.
- Out of scope: broader evolution workspace scoping, JSON status schema changes, and unrelated rendering cleanup.

## Acceptance Criteria

- [x] Human `evolution status` does not panic on long CJK/emoji event reasons.
- [x] Regression test covers the actual status rendering path, not only a helper.
- [x] Required Rust and task validation commands pass.

## Risks

- Risk: Changing preview length semantics could alter human-only output formatting.
  - Impact: Low; JSON output and persisted data are unchanged.
  - Mitigation: Keep the same ASCII preview threshold shape and only change the truncation mechanism.

## Validation Plan

- Required tests: focused `skilllite-commands` test, full workspace tests per policy.
- Commands to run: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test -p skilllite-commands`; `cargo test`; `python3 scripts/validate_tasks.py`.
- Manual checks: Re-read modified files and task board after updates.

## Regression Scope

- Areas likely affected: `skilllite evolution status` human recent-event output.
- Explicit non-goals: no evolution DB schema changes, no desktop JSON payload changes, no behavior changes for backlog/proposal commands.

## Links

- Source TODO section: N/A; found during critical bug investigation automation.
- Related PRs/issues: Recent UTF-8 truncation fixes `TASK-2026-066`, `TASK-2026-067`; workspace DB scoping fix `TASK-2026-068`.
- Related docs: N/A; user-facing semantics are unchanged.
