# TASK Card

## Metadata

- Task ID: `TASK-2026-049`
- Title: Desktop: show all installed skills
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-22`
- Target milestone:

## Problem

The desktop app currently lists only "scripted" skills, filtering out valid
installed skills that rely on `allowed-tools` or prompt-only behavior. This makes
successfully installed packages appear missing and also blocks related desktop
actions like opening or deleting those skills from the UI.

## Scope

- In scope:
  - Change desktop skill discovery to list all installed/discovered skill instances,
    not just script-backed ones.
  - Keep open-directory and remove-skill actions aligned with the same broader
    discovery logic.
  - Add regression coverage for non-script skills (for example a bash-tool or
    prompt-only skill).
- Out of scope:
  - Changing CLI installation behavior.
  - Reworking how skills execute at runtime.

## Acceptance Criteria

- [x] The desktop skill list includes installed skills that have `SKILL.md` but no
      script files, including `allowed-tools`/bash-tool style skills.
- [x] Desktop open-directory and delete actions work for those non-script skills
      because they reuse the same discovery path.
- [x] Regression tests cover at least one non-script skill discovery case.

## Risks

- Risk: Some desktop views may have assumed all listed skills are executable scripts.
  - Impact: UI actions could surface skills with different execution models.
  - Mitigation: Limit this task to list/open/delete behavior and keep execution
    semantics unchanged.

## Validation Plan

- Required tests:
  - `cargo test --manifest-path "crates/skilllite-assistant/src-tauri/Cargo.toml" list_skill_names`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm a previously hidden installed skill (such as `web-search`) appears in
    the desktop list after refresh / restart.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
- Explicit non-goals:
  - No changes to the actual installed skill files or manifest schema.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs`
