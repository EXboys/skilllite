# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/components/ChannelServeSettingsSection.tsx`
  - `crates/skilllite-assistant/src/components/SettingsModal.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
- Current behavior:
  - User-facing copy is already gateway-oriented, but the code still references `ChannelServeSettingsSection`, `settings.channelServe.*`, and `assistant_channel_health_probe`.

## Architecture Fit

- Layer boundaries involved:
  - UI-only Assistant naming cleanup plus a Tauri command rename.
- Interfaces to preserve:
  - Existing runtime health probing behavior.
  - Existing persisted settings fallback through legacy `channelServe*` fields.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Legacy persisted settings fields remain in the store for compatibility; this task is naming cleanup, not state removal.

## Design Decisions

- Decision:
  - Rationale:
  - Alternatives considered:
  - Why rejected:

## Open Questions

- [ ]
- [ ]
