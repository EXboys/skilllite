# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - `crates/skilllite-commands/src/schedule.rs`
  - `skilllite/src/cli.rs`
- Current behavior:
  - `check_schedule_due(&workspace)` reads the active workspace schedule in the Tauri host.
  - `spawn_rhythm` launches `skilllite schedule tick` without `--workspace`.
  - `cmd_tick(None, ...)` defaults to `"."`, canonicalized in the child process, so execution can diverge from the workspace used for due detection.

## Architecture Fit

- Layer boundaries involved:
  - Desktop/Tauri host starts the CLI subprocess.
  - CLI schedule command owns schedule execution.
- Interfaces to preserve:
  - Existing `skilllite schedule tick --workspace <path>` CLI contract.
  - Existing Life Pulse child environment merge.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Users who already set `SKILLLITE_WORKSPACE` in `.env` continue to work; the explicit CLI workspace now makes the UI-selected workspace authoritative.

## Design Decisions

- Decision: Add a small `rhythm_tick_args(workspace)` helper and use it in `spawn_rhythm`.
  - Rationale: Mirrors the existing `evolution_growth_args` pattern and is easy to unit test.
  - Alternatives considered: Set `SKILLLITE_WORKSPACE` in the rhythm env only.
  - Why rejected: `cmd_tick` already exposes explicit workspace selection, and explicit CLI args avoid dependence on dotenv/env merge details.

## Open Questions

- [x] Does schedule execution still require `SKILLLITE_SCHEDULE_ENABLED=1`? Yes, `cmd_tick` enforces it after due detection.
- [x] Does this require docs changes? No new user-facing CLI or behavior surface is introduced; the desktop host now uses an existing documented flag.
