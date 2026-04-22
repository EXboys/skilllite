# TASK Card

## Metadata

- Task ID: `TASK-2026-048`
- Title: Desktop: prefer workspace skilllite in debug
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-22`
- Target milestone:

## Problem

In desktop debug builds, the assistant currently prefers `~/.skilllite/bin/skilllite`
before the workspace-built binary. When a stale user-installed binary exists, the
desktop UI runs old CLI behavior that no longer matches the checked-out source,
causing confusing mismatches such as ZIP imports still being treated as plain local
paths.

## Scope

- In scope:
  - Change debug-mode subprocess resolution so the desktop app prefers the
    workspace `target/debug/skilllite` binary when it exists.
  - Keep bundled / home-bin / PATH fallbacks intact for other environments.
  - Add a regression-focused test for the workspace debug binary candidate path.
- Out of scope:
  - Release-mode path changes.
  - Changing CLI add/import behavior itself.
  - New desktop UI work.

## Acceptance Criteria

- [x] In debug builds, `resolve_skilllite_path_app()` checks the workspace-built
      `target/debug/skilllite` before `~/.skilllite/bin/skilllite`.
- [x] Existing bundled / home-bin / PATH fallback behavior remains available when
      the workspace debug binary is absent.
- [x] Tests cover the workspace debug binary candidate path shape so future changes
      do not silently revert the debug preference order.

## Risks

- Risk: Path derivation from `CARGO_MANIFEST_DIR` could be wrong if the crate layout changes.
  - Impact: Debug builds might fail to find the intended workspace binary.
  - Mitigation: Keep the helper small, test the expected suffix, and fall back to
    existing resolution paths when the candidate does not exist.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-assistant workspace_debug_skilllite`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - In a debug desktop run, verify ZIP import no longer hits stale old CLI behavior
    when a workspace-built `skilllite` exists.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
- Explicit non-goals:
  - No change to packaged desktop releases.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
