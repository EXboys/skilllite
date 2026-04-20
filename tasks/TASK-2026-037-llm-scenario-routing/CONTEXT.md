# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-assistant` (`useSettingsStore`, `buildAssistantBridgeConfig`, ChatView / StatusPanel / EvolutionSection).
- Current behavior: `buildAssistantBridgeConfig(settings)` sent the same LLM triple for every bridge call.

## Architecture Fit

- Layer boundaries involved: UI → Tauri IPC config payload only.
- Interfaces to preserve: Rust `ChatConfigOverrides` keys (`api_key`, `api_base`, `model`, …).

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Older persisted state without new keys behaves as routing off.

## Design Decisions

- Decision: Map scenarios to `LlmSavedProfile.id` instead of raw model strings.
  - Rationale: Reuses existing deduplication and quick-switch labels.
  - Alternatives considered: Duplicate model/base/key per scenario in settings.
  - Why rejected: Larger state and drift from saved profiles list.

## Open Questions

- [ ] Whether to prune stale route entries when a profile is deleted (deferred).
