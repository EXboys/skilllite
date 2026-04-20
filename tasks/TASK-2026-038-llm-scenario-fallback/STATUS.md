# Status Journal

## Timeline

- 2026-04-20:
  - Progress: Implemented `llmScenarioFallbacks` settings field, `runWithScenarioFallback` helper with cooldown + retryable-error classifier, wired into `followup` / evolution status (chat + EvolutionSection) / evolution manual trigger / Life Pulse LLM sync; added Settings UI fallback list editor; updated ZH/EN strings; updated assistant README and routing plan note.
  - Blockers: None.
  - Next step: None (MVP-A complete; Phase 2/3 deferred).
- 2026-04-20 (follow-up):
  - Progress: Added `runWithScenarioFallbackNotified` wrapper + ZH/EN toast strings; wired foreground call sites (`followup`, evolution status load in chat & EvolutionSection, evolution manual trigger). Background `lifePulse` LLM sync intentionally keeps the silent helper.
  - Blockers: None.
  - Next step: None.
- 2026-04-20 (UI polish):
  - Progress: Reworked the routing block in Settings into per-scenario collapsible cards (default-expanded only for scenarios with config). Empty fallback area is collapsed behind a "+ Add fallback profile" link. Streaming-chat note moved into the agent card body as a small italic line. Added ZH/EN strings for card summary, fallback badge tooltip, and add link.
  - Blockers: None.
  - Next step: None.
- 2026-04-20 (UI polish 2):
  - Progress: Replaced native scenario selects with a custom `appearance-none` style + chevron overlay (consistent with the existing evolution profile select). Empty fallback section is now fully hidden on reopen — no inline "+ Add" link, no "all added" hint. Discovery happens via a small "+" icon button in the card header that's only visible when the card is expanded and a fallback is not yet configured; clicking it reveals the editor for the current session. Added a "Cancel / 暂不配置" subtle link inside the empty editor to close it again. ZH/EN strings updated.
  - Blockers: None.
  - Next step: None.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed (`npm run build` in `crates/skilllite-assistant`)
- [x] Review complete
- [x] Board updated
