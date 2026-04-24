# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/stores/useSettingsStore.ts`
  - `crates/skilllite-assistant/src/components/GatewayServeSettingsSection.tsx`
- Current behavior:
  - The store has gateway fields plus legacy `channelServe*` fields for compatibility.
  - The gateway settings component still reads `settings.gatewayServe* ?? settings.channelServe*`.
  - There is no explicit persist version/migration yet.

## Architecture Fit

- Layer boundaries involved:
  - Assistant frontend store and UI only.
- Interfaces to preserve:
  - Existing users' local persisted values should still populate the gateway settings after migration.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Migration should copy old values into new fields on load; no manual reset should be required from users.

## Design Decisions

- Decision:
  - Rationale:
  - Alternatives considered:
  - Why rejected:

## Open Questions

- [ ]
- [ ]
