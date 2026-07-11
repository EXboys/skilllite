# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Fix Life Pulse rhythm workspace propagation
- Status: `done`
- Priority: `P1`
- Owner: `cursor`
- Contributors:
- Created: `2026-07-11`
- Target milestone:

## Problem

Desktop Life Pulse checks scheduled jobs against the active UI workspace, but the
background rhythm subprocess starts `skilllite schedule tick` without forwarding
that workspace. In packaged desktop environments the process current directory
is not the project workspace, so due scheduled jobs can be skipped even though
Life Pulse already detected them as due.

## Scope

- In scope:
  - Forward the active workspace to the rhythm subprocess.
  - Add a focused regression test for the constructed `schedule tick` arguments.
  - Validate formatting, task artifacts, and focused Rust tests.
- Out of scope:
  - Redesigning schedule execution or Life Pulse scheduling policy.
  - Changing CLI flags or documented command semantics.

## Acceptance Criteria

- [x] Life Pulse rhythm subprocess invokes `skilllite schedule tick --workspace <active workspace>`.
- [x] Existing growth subprocess behavior remains unchanged.
- [x] Regression test covers workspace propagation for rhythm arguments.
- [x] Task artifacts and board are updated with validation evidence.

## Risks

- Risk: Passing a relative workspace differently than prior implicit cwd behavior.
  - Impact: A desktop workspace stored as a relative path could resolve relative to the subprocess cwd.
  - Mitigation: Match the existing growth path by forwarding the same workspace string and keeping subprocess environment behavior unchanged.

## Validation Plan

- Required tests:
  - Focused `life_pulse` unit test for rhythm args.
  - Repository task artifact validation.
- Commands to run:
  - `cargo fmt --check`
  - `cargo test -p skilllite-assistant life_pulse::tests`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read modified files.
  - Confirm no docs update is required because no command, flag, env var, or user-facing documentation semantics changed.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - Desktop Life Pulse rhythm-triggered schedule execution.
- Explicit non-goals:
  - CLI `schedule tick` implementation.
  - Evolution growth scheduling.

## Links

- Source TODO section: N/A
- Related PRs/issues: Recent workspace scoping fixes around evolution and desktop subprocesses.
- Related docs: `spec/verification-integrity.md`, `spec/rust-conventions.md`, `spec/testing-policy.md`, `spec/docs-sync.md`
