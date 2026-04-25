# TASK Card

## Metadata

- Task ID: `TASK-2026-055`
- Title: Assistant minimal managed gateway process
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

The Assistant currently exposes gateway settings and a health probe, but users still have to manually run `skilllite gateway serve` in a separate terminal before the feature is usable. That adds friction and undermines the intended desktop UX.

This task adds the smallest useful managed-process layer so the desktop app can start, stop, and inspect a local `gateway serve` child process directly from the settings page.

## Scope

- In scope:
  - Add Tauri commands for gateway start/stop/status backed by shared child-process state.
  - Reuse the resolved `skilllite` binary path and start `gateway serve` as a child process.
  - Expose a minimal settings-page UI for one-click start/stop and visible runtime state.
  - Update relevant Assistant user-facing copy to describe desktop-managed startup.
- Out of scope:
  - Auto-start on app launch.
  - Crash auto-restart / watchdog behavior.
  - System service integration (`launchd`, `systemd`, Windows Service).
  - Full log streaming UI.

## Acceptance Criteria

- [x] Assistant can start `skilllite gateway serve` from the settings page without requiring a manual terminal command.
- [x] Assistant can stop a managed gateway child process from the settings page.
- [x] Settings UI shows a structured managed status (`running` / `stopped` / recent error) using Tauri status commands.
- [x] Frontend and Tauri validation/tests pass for the new behavior.

## Risks

- Risk:
  - Managed process state could diverge from reality if the child exits unexpectedly after startup.
  - Mitigation:
    - Status calls poll `try_wait()` and surface the latest stderr-derived error / exit code.

## Validation Plan

- Required tests:
  - Workspace Rust validation.
  - Assistant frontend production build.
  - Task document validation.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cd crates/skilllite-assistant && npm run build`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm the settings page exposes desktop-managed start/stop controls plus external command copy.
  - Confirm the managed status note clearly distinguishes desktop-owned vs external gateway processes.

## Regression Scope

- Areas likely affected:
  - Assistant Tauri command wiring.
  - Gateway settings page UX.
  - Desktop app shutdown cleanup for managed child processes.
- Explicit non-goals:
  - Auto-start and watchdog behavior remain out of scope.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
