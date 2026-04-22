# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src/components/StatusPanel.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-core/src/skill/{metadata.rs,deps.rs,manifest.rs}`
- Current behavior:
  - Desktop `skilllite_list_skills` currently returns only names.
  - The list UI shows just the name + checkbox + open-folder button.
  - Add success messages summarize count/source but do not surface missing setup.

## Architecture Fit

- Layer boundaries involved:
  - Desktop Tauri command boundary (`skilllite_list_skills`, `skilllite_add_skill`)
  - Shared metadata/manifest logic from `skilllite-core`
- Interfaces to preserve:
  - Existing add/remove/open command names.
  - Selected-skill operations keyed by skill name.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - `skilllite_list_skills` changes shape for the desktop frontend only; no external API contract.

## Design Decisions

- Decision: Upgrade `skilllite_list_skills` to return a richer desktop DTO instead
  of adding a second detail command.
  - Rationale: One fetch keeps the panel simple and ensures the list and details stay consistent.
  - Alternatives considered: Keep name-only list and add a separate detail lookup per selection.
  - Why rejected: More round-trips and more UI state complexity for little benefit.
- Decision: Limit missing setup hints to external prerequisites (commands/env vars).
  - Rationale: These are actionable and not already handled by SkillLite-managed env setup.
  - Alternatives considered: Guess whether Python/npm packages are installed.
  - Why rejected: Too noisy and less reliable.

## Open Questions

- [ ] Should future UI show install time / integrity state alongside trust?
- [ ] Should repair actions later be gated by skill type?
