# TASK Card

## Metadata

- Task ID: `TASK-2026-056`
- Title: Assistant detect externally managed gateway
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

The minimal managed gateway lifecycle added desktop-owned start/stop controls, but when the configured bind is already occupied by an externally started `skilllite gateway serve`, the settings page currently only surfaces a startup error. That is confusing because the desired gateway may already be healthy and usable.

This task teaches the Assistant to recognize a healthy externally managed gateway on the configured bind and present that as a distinct running state instead of a plain startup failure.

## Scope

- In scope:
  - Detect healthy external `skilllite gateway serve` listeners on the configured bind.
  - Distinguish `managed` vs `external` vs `stopped` states in the settings page status model.
  - Avoid surfacing a raw startup error when the configured bind is already served by an external healthy gateway.
  - Update Assistant-facing copy to explain the distinction.
- Out of scope:
  - Taking ownership of externally started processes.
  - Killing or supervising external gateway instances.
  - Auto-discovering external gateway processes on every port.

## Acceptance Criteria

- [x] Settings status explicitly distinguishes desktop-managed and externally running gateway instances on the configured bind.
- [x] Start action no longer surfaces only an error when a healthy external gateway is already serving the same bind.
- [x] Stop action remains limited to desktop-managed child processes.
- [x] Validation passes for Rust, frontend, and task artifacts.

## Risks

- Risk:
  - A random unrelated listener could occupy the same port and still cause confusing startup outcomes.
  - Mitigation:
    - External detection only upgrades the state when `GET /health` returns a valid SkillLite gateway-style `{ "ok": true }` response.

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
  - Confirm the settings page shows an external-running state when a healthy external gateway already serves the configured bind.
  - Confirm the stop control remains limited to desktop-managed processes.

## Regression Scope

- Areas likely affected:
- Explicit non-goals:

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
