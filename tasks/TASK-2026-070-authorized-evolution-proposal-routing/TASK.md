# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Route authorized evolution to the selected proposal
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors: Cursor automation
- Created: `2026-07-14`
- Target milestone: Next patch release

## Problem

The desktop authorization flow enqueues a specific capability-evolution proposal, but its
background `evolution run` omits `--proposal-id`. The CLI then removes the inherited force-proposal
environment variable, so the authorized proposal remains queued and an unrelated proposal may run.

## Scope

- In scope:
  - Pass the authorized proposal ID explicitly to the background CLI run.
  - Add a focused regression test for the spawned argument vector.
- Out of scope:
  - Refactoring evolution process spawning.
  - Changing coordinator policy or public CLI semantics.

## Acceptance Criteria

- [x] Authorized background runs include `--proposal-id <authorized-id>`.
- [x] Existing workspace targeting remains unchanged.
- [x] Focused and required repository tests pass; strict Clippy was run and hit one unrelated
  pre-existing Rust 1.97 lint in `skilllite-core`.

## Risks

- Risk: Incorrect argument ordering or ownership could prevent the background process from starting.
  - Impact: Authorized evolution would remain unavailable.
  - Mitigation: Assert the exact argument vector and run the assistant Rust test suite.

## Validation Plan

- Required tests: Focused argument-construction regression plus required Rust workspace checks.
- Commands to run:
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml authorized_run_args_include_target_workspace_and_proposal -- --nocapture`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
  - `python3 scripts/validate_tasks.py`
- Manual checks: Re-read the final spawn path and verify the CLI consumes `--proposal-id`.

## Regression Scope

- Areas likely affected: Desktop capability authorization and its background evolution subprocess.
- Explicit non-goals: Manual evolution trigger behavior, proposal selection policy, and CLI flag shape.

## Links

- Source TODO section: N/A — critical bug investigation.
- Related PRs/issues: Regression in the capability authorization flow reviewed around PR #101.
- Related docs: N/A — this restores already documented behavior.
