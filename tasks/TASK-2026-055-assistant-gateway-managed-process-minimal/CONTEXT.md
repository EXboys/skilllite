# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/components/GatewayServeSettingsSection.tsx`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/chat.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/README.md`
- Current behavior:
  - The settings page can copy a gateway command and run a local health probe.
  - The desktop app does not currently own the gateway child process lifecycle.
  - Chat subprocess management already exists via `Mutex<Option<Child>>` in Tauri and can serve as the local pattern reference.

## Architecture Fit

- Layer boundaries involved:
  - Assistant frontend ↔ Tauri command bridge ↔ `skilllite` child process.
- Interfaces to preserve:
  - `skilllite gateway serve` CLI surface remains unchanged.
  - Existing health probe behavior remains available.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Manual CLI startup should still remain possible; desktop-managed start is additive.

## Design Decisions

- Decision:
  - Rationale:
  - Alternatives considered:
  - Why rejected:

## Open Questions

- [ ]
- [ ]
