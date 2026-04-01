# TASK Card

## Metadata

- Task ID: `TASK-2026-014`
- Title: Evolution: add backlog query command
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone: `P7-D follow-up`

## Problem

There is no CLI entry to inspect evolution backlog proposals directly, so operators cannot quickly
query proposal state/risk/ROI after policy-runtime rollout.

## Scope

- In scope:
- Add `skilllite evolution backlog` command to query proposal backlog.
- Support practical filters (`status`, `risk`, `limit`) and sensible defaults.
- Show concise table output including proposal id, source, risk, status, ROI, acceptance status, update time.
- Wire command through CLI action enum and dispatch.
- Update EN/ZH user-facing command docs.
- Out of scope:
- Web dashboard or remote API.
- Schema migration for new backlog columns.

## Acceptance Criteria

- [x] `skilllite evolution backlog` runs and prints backlog rows from SQLite.
- [x] Filters by status/risk work correctly and are covered by tests.
- [x] Existing evolution commands remain unchanged and compile/test pass.
- [x] EN/ZH docs mention the new command.

## Risks

- Risk: Query output could be noisy when note text is very long.
  - Impact: CLI readability declines.
  - Mitigation: Truncate note preview and keep columns compact.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-commands`
  - `cargo test -p skilllite`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks:
  - Run `skilllite evolution backlog` with and without filters and verify outputs.

## Regression Scope

- Areas likely affected:
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - `crates/skilllite-commands/src/evolution.rs`
- Explicit non-goals:
  - No changes to execution policy semantics.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section `15.5.2`
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
