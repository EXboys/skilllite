# Status Journal

## Timeline

- 2026-04-20:
  - Progress: Task drafted from the Auto routing plan P0 list. Ready for implementation.
  - Blockers: Need to choose exact cleanup trigger points (load/save/delete).
  - Next step: Define the centralized cleanup helper and wire it into the selected lifecycle points.
- 2026-04-20 (implemented):
  - Progress: Added `cleanupLlmScenarioProfileReferences()` and `removeLlmProfileWithRoutingCleanup()` in `llmProfiles.ts`; hydration now auto-cleans stale `llmScenarioRoutes` / `llmScenarioFallbacks` in `MainLayout` after profile bootstrap; Settings delete and chat quick-switch delete both run cleanup immediately and show `toast.llmScenarioRefsCleaned` when references were pruned. README updated to document the self-healing behavior.
  - Blockers: None.
  - Next step: None for this task; `TASK-2026-041` remains ready.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
