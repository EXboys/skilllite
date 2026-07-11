# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - `crates/skilllite-commands/src/schedule.rs`
  - `skilllite/src/cli.rs`
- Current behavior:
  - `check_schedule_due` checks the active workspace path.
  - `spawn_rhythm` invokes `skilllite schedule tick` without `--workspace` or `current_dir`.
  - `cmd_tick` defaults missing workspace to `"."`, so desktop cwd can decide which schedule file is used.

## Architecture Fit

- Layer boundaries involved:
  - Tauri host spawns the CLI binary for heavy work.
  - CLI schedule command owns schedule parsing and state updates.
- Interfaces to preserve:
  - Existing `schedule tick --workspace` CLI interface.
  - Existing dotenv and LLM override environment propagation.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - Direct CLI users keep the same defaults.
  - Desktop Life Pulse becomes more deterministic by passing the workspace it already used for the due check.

## Design Decisions

- Decision: Add a small helper that builds rhythm arguments with the active workspace.
  - Rationale: Mirrors the existing `evolution_growth_args` pattern and is easy to unit test.
  - Alternatives considered: Set subprocess `current_dir` only.
  - Why rejected: `schedule tick` already has an explicit workspace flag, and explicit arguments avoid relying on inherited process state.

## Open Questions

- [x] Is a docs update required? No command, flag, env var, or documented default changes; this fixes desktop's internal invocation.
