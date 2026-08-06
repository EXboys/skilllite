# TASK-2026-083: Fix Life Pulse periodic growth anchor never advancing

## Metadata

- Task ID: `TASK-2026-083`
- Title: Fix Life Pulse periodic growth anchor never advancing
- Status: `in_progress`
- Priority: `P0`
- Owner: `cursor-cloud`
- Contributors:
- Created: `2026-08-06`
- Target milestone:
- Branches: `cursor/critical-bug-investigation-b649`

## Summary

Desktop Life Pulse growth uses read-only `evolution status --json` / `inspect_growth_due` but never seeds or advances `last_periodic_growth_unix`. With a `None` anchor, `inspect_growth_due` treats elapsed as 0 every heartbeat, so the periodic arm never fires. Agent-rpc still uses mutating `growth_due` and is unaffected.

## Acceptance Criteria

- [x] `evolution_growth_due` seeds the periodic mutex on first heartbeat (`None` → `Some(now)`).
- [x] When A9 reports `arm_periodic` on a due tick, the mutex advances to `now` (including empty-proposal skip path).
- [x] Unit tests cover seed-once / advance-on-arm / interval semantics matching `growth_due`.
- [x] Task artifacts + board updated; `python3 scripts/validate_tasks.py` passes.

## Non-Goals

- Rhythm/`schedule tick` workspace scoping (open PRs #115/#116).
- Signal/sweep arms (already work without the mutex).
- Changing CLI `inspect_growth_due` semantics.

## Risks

- Holding/updating the mutex incorrectly could spam `evolution run` every 30s after the first interval. Mitigated by advancing on `arm_periodic` and unit tests for interval reset.

## Validation Plan

- Unit tests in `evolution_ui/growth.rs`.
- `cargo test` for the assistant crate (or targeted lib tests if Tauri deps allow).
- Clippy/fmt on touched files.

## Regression Scope

- Life Pulse growth heartbeat scheduling.
- Evolution status UI reading `periodic_anchor_unix` from the same mutex.
