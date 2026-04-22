# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
- Current behavior:
  - `resolve_skilllite_path_app()` prefers `~/.skilllite/bin/skilllite` in debug builds.
  - The desktop add-skill command uses that resolver, so a stale user-installed binary
    can override freshly built workspace CLI behavior.

## Architecture Fit

- Layer boundaries involved:
  - Desktop frontend/backend boundary unchanged.
  - Only subprocess path selection in the desktop bridge changes.
- Interfaces to preserve:
  - `resolve_skilllite_path_app(app)`
  - Existing fallback-to-bundled / PATH behavior.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Debug builds change preference order only.
  - Release builds should keep current behavior.

## Design Decisions

- Decision: Derive a workspace debug binary candidate from `CARGO_MANIFEST_DIR`
  and prefer it only under `debug_assertions`.
  - Rationale: Keeps the fix local, deterministic, and aligned with the workspace layout.
  - Alternatives considered: Add an environment-variable override only.
  - Why rejected: Would still leave the default debug experience surprising.

## Open Questions

- [ ] Should we later add an explicit env override for power users to force a custom subprocess binary?
- [ ] Should chat-mode resolution share a common helper with app-mode resolution in a follow-up task?
