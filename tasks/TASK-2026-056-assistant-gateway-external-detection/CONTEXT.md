# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/gateway_manager.rs`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src/components/GatewayServeSettingsSection.tsx`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/README.md`
- Current behavior:
  - The settings page can show desktop-managed gateway state and start/stop a child process.
  - If the same bind is already occupied by an external gateway, the managed start path currently reports a startup failure after the child exits.
  - Health checks already exist and can validate whether a gateway is healthy on the configured bind.

## Architecture Fit

- Layer boundaries involved:
  - Assistant frontend ↔ Tauri status bridge ↔ local gateway health probe / managed child state.
- Interfaces to preserve:
  - The stop action only applies to the desktop-owned child process.
  - Existing manual terminal/systemd gateway flows remain supported.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Existing managed-process behavior remains intact; external detection is additive and non-invasive.

## Design Decisions

- Decision:
  - Rationale:
  - Alternatives considered:
  - Why rejected:

## Open Questions

- [ ]
- [ ]
