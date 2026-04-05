# TASK Card

## Metadata

- Task ID: `TASK-2026-022`
- Title: Desktop IDE three-pane layout
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-05`
- Target milestone:

## Problem

Desktop users wanted a Cursor-like layout: file tree, editor, and chat in one window instead of only session list + chat + status panel.

## Scope

- In scope: Tauri commands to list/read workspace files (aligned with existing write rules), React layout toggle, CodeMirror editor column, settings + header control, docs note.
- Out of scope: Panel resize drag handles, opening binary files, full VS Code parity, replacing external IDE.

## Acceptance Criteria

- [x] User can enable IDE layout from header and Settings; preference persists.
- [x] Left column shows workspace file tree (with Sessions tab) and skips heavy dirs; selecting a file opens it in the center editor with save.
- [x] Right column shows chat; standard layout unchanged when IDE layout is off.
- [x] Path safety matches `write_workspace_text_file` (canonical root, sensitive paths blocked for read too).

## Risks

- Risk: Large repos may still be slow to list (cap 5000 entries, depth 14).
  - Impact: UI freeze during list.
  - Mitigation: spawn_blocking in Tauri; refresh is manual + on save.

## Validation Plan

- Required tests: `workspace_path_tests` in `workspace.rs`
- Commands to run: `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml workspace_path_tests`, `npm run build` in `crates/skilllite-assistant`, `cargo clippy --manifest-path ...`
- Manual checks: Toggle IDE, open file, save, switch workspace clears selection.

## Regression Scope

- Areas likely affected: `MainLayout`, settings persistence, Tauri invoke handler list.
- Explicit non-goals: Status panel in IDE mode (hidden; documented).

## Links

- Related docs: `crates/skilllite-assistant/README.md`, root `README.md`, `docs/zh/README.md`
