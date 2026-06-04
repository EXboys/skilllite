# TASK Card

## Metadata

- Task ID: `TASK-2026-067`
- Title: Fix authorized evolution proposal run binding
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors: `automation`
- Created: `2026-06-04`
- Target milestone: recent critical bug investigation

## Problem

Desktop chat authorization for capability evolution enqueues a concrete backlog proposal and then starts a background `skilllite evolution run`. The background run only sets `SKILLLITE_EVO_FORCE_PROPOSAL_ID`, but `cmd_run` clears that environment variable whenever no `--proposal-id` CLI argument is present. As a result, the user-authorized proposal may remain queued while the background run sees no forced proposal and either does nothing or selects a different candidate.

## Scope

- In scope: bind the authorized background evolution run to the returned proposal id using the existing `--proposal-id` CLI contract.
- In scope: add a focused regression test for the background argument construction.
- Out of scope: changing evolution policy runtime decisions, backlog schema, or LLM synthesis behavior.

## Acceptance Criteria

- [ ] Background runs started by `skilllite_authorize_capability_evolution` pass `--proposal-id <authorized id>`.
- [ ] The forced proposal environment variable is no longer the only binding mechanism for this path.
- [ ] Tests cover the argument contract and the affected crates still pass required checks.

## Risks

- Risk: argument construction diverges from manual trigger behavior again.
  - Impact: authorized proposals can be skipped or unrelated proposals can execute.
  - Mitigation: centralize the authorization background run args in a small helper with a regression test.

## Validation Plan

- Required tests: targeted Rust tests for the assistant bridge and workspace Rust tests.
- Commands to run: `cargo fmt --check`; `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml authorize_background_run_args_force_proposal`; `cargo test -p skilllite-commands`; `cargo test`.
- Manual checks: inspect the final `authorize.rs` command chain to confirm the child receives `--proposal-id`.

## Regression Scope

- Areas likely affected: desktop chat "start evolution" action, desktop evolution CLI bridge, `skilllite evolution run` proposal forcing.
- Explicit non-goals: policy runtime thresholds, proposal recovery, queue selection, and UI copy.

## Links

- Source TODO section: N/A, triggered by automated recent-commit bug investigation.
- Related PRs/issues: recent P2 desktop/CLI split commits.
- Related docs: N/A, this restores documented CLI behavior without changing user-facing commands.
