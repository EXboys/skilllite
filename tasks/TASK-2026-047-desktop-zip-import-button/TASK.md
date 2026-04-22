# TASK Card

## Metadata

- Task ID: `TASK-2026-047`
- Title: Desktop: add ZIP import picker
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-22`
- Target milestone:

## Problem

The desktop app can already install a local ZIP skill by pasting the file path
into the generic add-source input, but that workflow is hidden and unfriendly.
Users need a first-class file picker button so ZIP-based skill import is
discoverable without manual path copying.

## Scope

- In scope:
  - Add a ZIP file picker button in the desktop `StatusPanel` skill-add area.
  - Reuse the existing `skilllite_add_skill` Tauri command by passing the selected
    file path as the add source.
  - Update EN/ZH desktop UI copy so the add-source placeholder and button labels
    mention ZIP packages.
- Out of scope:
  - New backend commands or changes to the `skilllite add` ZIP import path.
  - Drag-and-drop import or remote ZIP URL flows.
  - Multi-file bulk ZIP import.

## Acceptance Criteria

- [x] The desktop skill panel offers an explicit button to choose a local `.zip`
      file and routes the chosen path through the existing add flow.
- [x] Cancelled file-picking is a no-op; picker/open failures surface a readable
      error in the existing add-result area without crashing the panel.
- [x] EN/ZH UI text reflects that the add flow accepts repo sources and local ZIP
      packages.

## Risks

- Risk: The dialog plugin can return different value shapes across platforms.
  - Impact: The picker button might break on macOS/Windows path selection.
  - Mitigation: Accept both string and string-array responses defensively, while
    requesting a single file selection.

## Validation Plan

- Required tests:
  - Read lints for the touched desktop files.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Open the desktop skill panel, click the ZIP import button, choose a local
    ZIP file, and confirm the selection populates the add/install flow.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src/components/StatusPanel.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
- Explicit non-goals:
  - Modifying the CLI ZIP import implementation added in `TASK-2026-046`.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
