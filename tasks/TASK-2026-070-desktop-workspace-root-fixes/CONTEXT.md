# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/{paths,transcript,sessions,workspace}.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/prompt_artifact.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - Desktop frontend invoke callsites under `crates/skilllite-assistant/src/`
- Current behavior:
  - `chat_stream` writes through `agent-rpc` with `current_dir=<project root>` and `SKILLLITE_WORKSPACE=<project root>`.
  - Host helpers such as `load_transcript`, `list_sessions`, memory/log reads, and prompt artifact helpers still use `local::chat_root()` through the process environment.
  - Life Pulse `check_schedule_due` checks the active workspace, but `spawn_rhythm` calls `schedule tick` without `--workspace` or `current_dir`.

## Architecture Fit

- Layer boundaries involved:
  - Tauri command layer remains an entry layer over the desktop bridge.
  - The desktop bridge continues to delegate runtime execution to the `skilllite` subprocess.
- Interfaces to preserve:
  - Optional workspace parameters for desktop commands.
  - Existing process-global fallback when no workspace is supplied.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - Backend command defaults preserve omitted-workspace behavior.
  - Frontend callsites pass the active workspace where available.

## Design Decisions

- Decision: Add one chat-root helper that accepts an optional workspace and otherwise falls back to the legacy global root.
  - Rationale: It localizes the compatibility rule and makes read/write callsites explicit.
  - Alternatives considered: Mutating the Tauri process environment when the UI workspace changes.
  - Why rejected: Process-global environment mutation would be race-prone and unsafe for multiple windows/workspaces.
- Decision: Make rhythm arguments mirror growth by passing `--workspace` and setting `current_dir`.
  - Rationale: Due detection and execution must share the same root to avoid wrong-project agent runs.
  - Alternatives considered: Relying only on `SKILLLITE_WORKSPACE` in environment pairs.
  - Why rejected: `schedule tick` already exposes an explicit workspace argument and current-dir scoping handles relative paths consistently.

## Open Questions

- [x] Should CLI-only evolution maintenance commands be fixed here? No; they are tracked as an adjacent known issue and are outside this desktop-focused PR.
