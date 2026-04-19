# TASK Card

## Metadata

- Task ID: `TASK-2026-035`
- Title: Assistant full-page settings and environment deps
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-19`
- Target milestone:

## Problem

Settings were modal-only and Python/Node/Git tooling lived in the session sidebar, making dependency management hard to discover.

## Scope

- In scope:
  - Full-page settings layout with left navigation instead of centered modal.
  - New **Environment** section: Git probe (`skilllite_git_status`), Python/Node provisioning reuse (existing Tauri hooks).
  - Session sidebar shortcut to Environment settings; toolbar shows “Back to chat” while settings open.
  - Backend: `skilllite_git_status` command.
- Out of scope:

  - Silent/portable Git install bundled into the app.

## Acceptance Criteria

- [x] Opening Settings shows a full main-area layout (not a floating dialog) with navigable sections.
- [x] Environment section lists Git status + Python/Node runtime actions (download/refresh).
- [x] Session sidebar links to Environment settings; EN/ZH strings updated.
- [x] `cargo check` for `skilllite-assistant` Tauri crate and `npx tsc -b` pass.

## Risks

- Risk: Users miss “Save” while expecting auto-save for environment actions.
  - Impact: Low — runtime download is immediate; Git is detect-only.
  - Mitigation: Copy existing provision UX from sidebar.

## Validation Plan

- Required tests: manual smoke in desktop app.
- Commands to run:
  - `cd crates/skilllite-assistant/src-tauri && cargo check`
  - `cd crates/skilllite-assistant && npx tsc -b`
  - `python3 scripts/validate_tasks.py`
- Manual checks: Open Settings → Environment; verify Git line and provision buttons.

## Regression Scope

- Areas likely affected: `MainLayout`, `SettingsModal`, `SessionSidebar`, Tauri command list.
- Explicit non-goals: Changing skill install CLI behavior.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `crates/skilllite-assistant/README.md`
