# Technical Context

## Current State

- Relevant files/modules:
  - `crates/skilllite-assistant/src/utils/llmProfiles.ts`
  - `crates/skilllite-assistant/src/components/MainLayout.tsx`
  - `crates/skilllite-assistant/src/components/SettingsModal.tsx`
  - `crates/skilllite-assistant/src/components/ChatView.tsx`
  - `crates/skilllite-assistant/README.md`
- Previous behavior: Missing ids were skipped when building candidate chains, but persisted settings could still contain stale primary/fallback references. Deleting a saved profile only reselected the current LLM session, leaving routing references untouched.
- Implemented behavior: `cleanupLlmScenarioProfileReferences()` now centralizes stale-reference pruning; `removeLlmProfileWithRoutingCleanup()` combines session reselection with routing cleanup. `MainLayout` runs the cleanup once hydration completes (and after bootstrap profile creation), while Settings and chat quick-switch deletions run the cleanup immediately and show an info toast when stale references are removed.

## Architecture Fit

- Layer boundaries involved: assistant persistence + UI only.
- Interfaces to preserve: `Settings` stays backward compatible; stale ids self-heal into cleaned `llmScenarioRoutes` / `llmScenarioFallbacks`.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Older persisted state with stale ids now self-heals on startup.

## Design Decisions

- Decision: Centralize cleanup in `llmProfiles.ts`.
  - Rationale: The same cleanup rules are needed on load and on profile delete.
  - Alternatives considered: Filter only at render time or only inside routing helpers.
  - Why rejected: UI-only filtering hides the bug without repairing persisted state; routing-only filtering leaves ghost references in settings.

- Decision: Show a minimal info toast when cleanup removes stale references.
  - Rationale: Gives users a clue that the system corrected invalid config instead of silently changing state.
  - Alternatives considered: Silent cleanup.
  - Why rejected: Harder to understand why a route/fallback disappeared after deletion or upgrade.

## Open Questions

- [ ] Whether cleanup should also run on Settings save to normalize any hand-edited persisted state immediately.
- [ ] Whether the toast should include scenario names instead of only counts.
