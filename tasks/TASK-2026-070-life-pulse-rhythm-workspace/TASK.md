# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Bind Life Pulse rhythm ticks to active workspace
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-12`
- Target milestone:

## Problem

Life Pulse checks whether scheduled jobs are due against the active desktop workspace, but the rhythm subprocess launches `skilllite schedule tick` without passing that workspace. In a packaged desktop app, or any environment where the process current directory is not the selected project, the due check and execution path can point at different `.skilllite/schedule.json` files. This can silently skip due scheduled agent jobs or execute jobs from the wrong workspace.

## Scope

- In scope:
  - Pass the active workspace to the Life Pulse rhythm subprocess via the existing `schedule tick --workspace` CLI.
  - Add a focused regression test for the rhythm argument builder.
  - Record validation evidence and regression scope.
- Out of scope:
  - Changing schedule file semantics or state persistence.
  - Changing Life Pulse growth/evolution behavior.
  - Changing desktop settings UI or adding new configuration.

## Acceptance Criteria

- [ ] Life Pulse rhythm subprocess invokes `skilllite schedule tick --workspace <active workspace>`.
- [ ] Existing environment merge behavior for rhythm subprocesses is preserved.
- [ ] Regression coverage proves spaces in workspace paths remain a single argument.
- [ ] Task artifacts and board are updated and validated.

## Risks

- Risk: Passing a raw workspace string could alter behavior for users relying on process current directory.
  - Impact: Schedule tick would now use the UI-selected workspace, which is the intended Life Pulse contract.
  - Mitigation: Use the existing CLI `--workspace` interface and preserve current child environment handling.

## Validation Plan

- Required tests:
  - Focused assistant crate unit test for rhythm args.
  - Task artifact validation.
- Commands to run:
  - `cargo fmt --check`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml life_pulse::tests::rhythm_args_include_active_workspace`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read changed source and task files after edits.

## Regression Scope

- Areas likely affected:
  - Desktop Life Pulse rhythm subprocess startup.
  - Scheduled agent runs launched from desktop heartbeat.
- Explicit non-goals:
  - CLI schedule behavior outside Life Pulse.
  - Evolution growth subprocess behavior.

## Links

- Source TODO section: N/A
- Related PRs/issues: Recent workspace scoping fixes around evolution and Life Pulse.
- Related docs: `skilllite/src/cli.rs` documents `skilllite schedule tick --workspace /path/to/project`.
