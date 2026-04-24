# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/components/ChannelServeSettingsSection.tsx`
  - `crates/skilllite-assistant/src/stores/useSettingsStore.ts`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/README.md`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs` (`assistant_channel_health_probe`)
  - `TASK-2026-051-gateway-phase1-bootstrap`
- Current behavior:
  - The Assistant page is still labeled around `channel serve`, stores `channelServeBind` / `channelServeToken`, and generates `SKILLLITE_CHANNEL_SERVE_ALLOW=1 skilllite channel serve ...`.
  - Health probing already uses a native Tauri command against loopback `/health`, which is generic enough for the new gateway host.
  - The page has no artifact-dir field, so it cannot generate a complete `gateway serve` command for unified hosting.

## Architecture Fit

- Layer boundaries involved:
  - This task is UI/persistence only; gateway backend behavior from `TASK-2026-051` must remain unchanged.
  - Tauri health probing remains an Assistant host concern and should not require new Rust server logic.
- Interfaces to preserve:
  - Existing settings tab navigation (`channel` tab id can stay stable even if wording changes).
  - Existing health-check command shape (`assistant_channel_health_probe`).
  - Existing stored bind/token values should remain readable.

## Dependency and Compatibility

- New dependencies:
  - None expected.
- Backward compatibility notes:
  - New gateway-specific settings keys may be added, but old `channelServe*` values should be used as fallback to avoid data loss.

## Design Decisions

- Decision: Migrate the page in place instead of renaming the entire settings tab/component tree first.
  - Rationale: The user wants the operating model changed now; tab-id churn and larger refactors add little value.
  - Alternatives considered:
    - Rename all `channel*` identifiers immediately.
    - Add a second separate gateway page and deprecate the old one later.
  - Why rejected:
    - Full rename churn is larger than needed for one task.
    - Two pages would increase confusion and duplicate state.

- Decision: Add explicit `artifact-dir` configuration to the page.
  - Rationale: Without it, the settings UI would still underrepresent the new unified host.
  - Alternatives considered:
    - Migrate wording only and omit artifact support.
  - Why rejected:
    - That would not fully reflect the gateway host capability the user just asked for.

- Decision: Keep the existing native health probe command and only point it at gateway `/health`.
  - Rationale: The backend probe is already loopback-limited and generic enough.
  - Alternatives considered:
    - Rename or replace the Tauri command immediately.
  - Why rejected:
    - Renaming adds churn without changing runtime behavior.

## Open Questions

- [ ]
- [ ] Should persisted legacy `channelServe*` keys be kept indefinitely or cleaned up in a later migration task?
- [ ] After this UI migration, should the settings tab label remain “Inbound Webhook” or broaden to “Gateway / Inbound HTTP” in a follow-up?
