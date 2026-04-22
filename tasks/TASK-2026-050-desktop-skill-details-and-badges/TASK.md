# TASK Card

## Metadata

- Task ID: `TASK-2026-050`
- Title: Desktop: skill badges and dependency hints
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-22`
- Target milestone:

## Problem

After making all installed skills visible, the desktop list still gives too
little context: users cannot quickly distinguish skill type, inspect source /
trust / dependencies, or understand missing post-install setup. ZIP-installed
skills like `web-search` need clearer at-a-glance metadata and immediate setup
warnings.

## Scope

- In scope:
  - Enrich desktop skill list data with type, source, trust, dependencies, and
    missing setup hints.
  - Show type badges in the skills list and a details panel for the selected skill.
  - Append post-install missing dependency hints to the add-result message.
  - Update EN/ZH desktop docs and UI copy.
- Out of scope:
  - Changing runtime execution semantics for different skill types.
  - Full marketplace metadata / screenshots in the desktop app.

## Acceptance Criteria

- [x] The desktop skills list shows a type badge for each skill (`Script`,
      `Bash Tool`, `Prompt Only`).
- [x] When a single skill is selected, the panel shows source, trust, and
      dependency details.
- [x] After adding a skill, the add result includes missing command / env hints
      when setup is still incomplete.

## Risks

- Risk: Richer list DTOs can drift from backend metadata if parsed inconsistently.
  - Impact: UI could show misleading type or setup hints.
  - Mitigation: Derive the UI DTO directly from existing core metadata, manifest,
    and dependency helpers; add regression tests.

## Validation Plan

- Required tests:
  - `cargo test --manifest-path "crates/skilllite-assistant/src-tauri/Cargo.toml" list_skill_names`
  - `cargo test --manifest-path "crates/skilllite-assistant/src-tauri/Cargo.toml" summarise_add_output`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cd crates/skilllite-assistant && npm run build`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Select an installed skill in the desktop panel and confirm badge/details render.
  - Add a skill with missing prerequisites and confirm the add-result warning appears.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src/components/StatusPanel.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
- Explicit non-goals:
  - No manifest schema or install command changes.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
