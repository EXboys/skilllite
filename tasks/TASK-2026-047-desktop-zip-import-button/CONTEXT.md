# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/components/StatusPanel.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
- Current behavior:
  - The desktop panel already has a text input and Add button that call
    `invoke("skilllite_add_skill", { source, ... })`.
  - Tauri already depends on `@tauri-apps/plugin-dialog`, and other desktop
    components use native directory / image pickers.
  - Backend ZIP import support already exists via `skilllite add <local-zip>`.

## Architecture Fit

- Layer boundaries involved:
  - Frontend React component state in `StatusPanel`.
  - Existing Tauri command boundary `skilllite_add_skill`.
- Interfaces to preserve:
  - `skilllite_add_skill(workspace, source, force)`
  - Existing manual source text entry flow.

## Dependency and Compatibility

- New dependencies:
  - None expected; reuse existing dialog plugin import.
- Backward compatibility notes:
  - Repo/local-path add should continue to work unchanged.
  - ZIP picker should be additive, not replace the text box.

## Design Decisions

- Decision: Add a separate ZIP picker button next to the source input instead of
  auto-opening a picker from the Add button.
  - Rationale: Keeps repo/path text input intact while making ZIP import explicit.
  - Alternatives considered: Replace the text input with a picker-first flow.
  - Why rejected: Would regress existing repo-based install ergonomics.
- Decision: Surface picker errors through the existing add-result banner.
  - Rationale: Reuses current feedback UI and avoids adding another toast-only path.
  - Alternatives considered: Only show picker failures via toast.
  - Why rejected: Easy to miss and duplicates panel-level result handling.

## Open Questions

- [ ] Should the picker also allow `.skill.zip` / marketplace-specific extensions later?
- [ ] Do we want drag-and-drop import in a later task?
