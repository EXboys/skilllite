# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/stores/useSettingsStore.ts`
  - `crates/skilllite-assistant/src/utils/llmScenarioRouting.ts`
  - `crates/skilllite-assistant/src/utils/llmScenarioFallback.ts` (new)
  - `crates/skilllite-assistant/src/components/SettingsModal.tsx`
  - `crates/skilllite-assistant/src/components/ChatView.tsx`
  - `crates/skilllite-assistant/src/components/EvolutionSection.tsx`
  - `crates/skilllite-assistant/src/components/StatusPanel.tsx`
- Current behavior: Scenario routing replaces the bridge config's LLM fields per call site, but a failure surfaces directly to the UI.

## Architecture Fit

- Layer boundaries involved: Assistant TypeScript only; Tauri IPC payloads stay backwards-compatible.
- Interfaces to preserve: Rust `ChatConfigOverrides` shape; existing scenario keys.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Old persisted state lacking `llmScenarioFallbacks` is treated as "no fallbacks". Streaming chat behavior unchanged.

## Design Decisions

- Decision: Implement fallback in the frontend rather than the Rust bridge.
  - Rationale: All non-streaming endpoints in scope already accept full `config` per call; reusing existing IPC surface avoids cross-crate churn.
  - Alternatives considered: Push fallback into the Rust agent.
  - Why rejected: Higher blast radius, requires shipping a new agent build, and the streaming path needs deeper changes that are explicitly out of scope.

- Decision: Heuristic substring match on raw error string.
  - Rationale: Tauri serializes Rust errors as plain strings here; structured error codes would require a coordinated bridge change.
  - Alternatives considered: Add a Rust-side typed error envelope.
  - Why rejected: Out of scope for MVP-A; can be revisited when streaming fallback lands.

- Decision: In-process `Map` for cooldown.
  - Rationale: User chose simplest viable cooldown; cross-session persistence isn't needed for the immediate goal of avoiding rapid re-hammering.
  - Alternatives considered: Persist to `localStorage`.
  - Why rejected: Adds complexity without addressing the dominant failure mode.

## Open Questions

- [ ] Whether to surface the "switched to fallback" event in the UI (currently silent; Phase 2 / observability layer).
- [ ] Whether to extend fallback to streaming `agent` chat; would require restarting the Tauri stream cleanly on early failure.
