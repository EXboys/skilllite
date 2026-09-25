# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/**` (Tauri 2 + React; excluded from root workspace)
  - `crates/skilllite-assistant/scripts/prebuild-skilllite.sh` (assumes `../../..` is engine root)
  - `src-tauri/src/skilllite_bridge/paths.rs` (`../../../target/debug` from `src-tauri`)
  - `.github/workflows/release-desktop.yml`, `ci.yml` deny job, root `package.json`
- Current behavior:
  - Assistant Cargo.toml has zero `skilllite-*` path dependencies
  - Prebuild `cd`s to monorepo root and `cargo install --path skilllite`

## Architecture Fit

- Layer boundaries involved: entry (desktop) must remain subprocess-only over engine binary
- Interfaces to preserve: L1 `agent-rpc`, L2 `--json`, L3 workspace files

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes:
  - Bookmarks to `crates/skilllite-assistant` break; stub README explains the move
  - `SKILLLITE_ENGINE_ROOT` is the supported override when the engine checkout is not a parent/sibling

## Design Decisions

- Decision: Relocate to top-level `skilllite-assistant/` in this repository rather than deleting the tree
  - Rationale: This environment cannot create a new GitHub remote; keeping a complete project tree avoids losing the app
  - Alternatives considered: Delete + external repo only; leave under `crates/`
  - Why rejected: External-only extract would drop sources; `crates/` still implies an engine crate
- Decision: Discover engine root by env + walk for `skilllite/Cargo.toml`
  - Rationale: Works for monorepo and for a future split checkout next to the engine
  - Alternatives considered: Hard-code `../..` after the move
  - Why rejected: Breaks as soon as the project is a true separate repo

## Open Questions

- [x] Separate GitHub repo in this PR? No — in-tree standalone project only.
- [x] Vendor extra bundled skills? Keep existing `src-tauri/resources/bundled-skills`; refresh from engine `.skills` when present.
