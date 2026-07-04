# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/chat.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/*.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
- Current behavior:
  - `chat_stream` calls `find_project_root(raw_workspace)` and injects that path into
    `SKILLLITE_WORKSPACE`.
  - Evolution UI calls pass raw `workspace` strings to CLI `--workspace`, so CLI DB and skill root
    resolution can point at a nested directory instead of the project root.

## Architecture Fit

- Layer boundaries involved:
  - Desktop bridge entry layer builds CLI subprocess arguments.
  - Lower-layer CLI/commands continue to accept `--workspace` without depending on desktop code.
- Interfaces to preserve:
  - Existing Tauri command signatures continue to accept raw workspace strings.
  - `spawn_skilllite_json` continues to receive raw workspace for dotenv discovery.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - Users who configured nested app directories under a parent skill root will now see evolution
    data align with chat data at the parent root.
  - Workspaces without a parent skill root keep their original path.

## Design Decisions

- Decision: Canonicalize only the CLI `--workspace` argument at the desktop evolution boundary.
  - Rationale: This directly aligns evolution DB/skill roots with chat/A9 while preserving raw
    workspace dotenv lookup.
  - Alternatives considered: Change lower-layer CLI `resolve_workspace_root` to walk parents.
  - Why rejected: CLI users may intentionally pass exact paths; the observed split is created by
    desktop bridge mixing canonical chat with raw evolution arguments.

## Open Questions

- [x] Should CLI-only reset/disable/explain be changed in this task? No; that is a separate command
  surface design and is outside this targeted desktop split-brain fix.
