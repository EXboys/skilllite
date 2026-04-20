# TASK Card

## Metadata

- Task ID: `TASK-2026-038`
- Title: Local LLM scenario runtime fallback (MVP-A)
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Scenario routing (TASK-2026-037) lets users pin a saved profile per call site, but a 429/5xx/timeout/network error from that profile would still surface as a hard failure. Per the `todo/assistant-auto-llm-routing-plan.md` Phase 1 / MVP-A, we want non-streaming bridge calls to auto-switch to the next saved profile when the primary returns a retryable error, with a short process-local cooldown so a bad provider isn't hammered repeatedly.

## Scope

- In scope:
  - Per-scenario ordered fallback profile list in `Settings`.
  - `runWithScenarioFallback` helper with retryable-error classification + in-memory cooldown.
  - Wire the helper into non-streaming invokes: `skilllite_followup_suggestions`, `skilllite_load_evolution_status` (chat + EvolutionSection), `skilllite_trigger_evolution_run`, `skilllite_life_pulse_set_llm_overrides`.
  - Settings UI for managing the fallback list per scenario, plus ZH/EN strings.
  - Assistant README sub-bullet (ZH + EN) and todo plan status note.
- Out of scope:
  - Mid-stream switching for `skilllite_chat_stream` (streaming `agent`).
  - Persistent / cross-session cooldown.
  - Heuristic complexity-based routing.
  - Health-score based provider scoring.

## Acceptance Criteria

- [x] `Settings.llmScenarioFallbacks` persists in `localStorage` alongside existing routing keys.
- [x] When routing is enabled and the primary fails with a retryable error, the next fallback profile is tried automatically.
- [x] Non-retryable errors (auth, schema, missing key) are propagated without switching.
- [x] A failed candidate enters a 60 s in-memory cooldown and is skipped during that window.
- [x] Streaming `agent` chat behavior is unchanged.
- [x] `npm run build` passes in `crates/skilllite-assistant`.

## Risks

- Risk: Heuristic substring match on Tauri error messages misclassifies an error.
  - Impact: Either a transient error fails fast (no switch) or an auth error switches profiles silently.
  - Mitigation: Patterns are conservative (only well-known transient phrases); unrecognized errors are treated as non-retryable.

## Validation Plan

- Required tests: TypeScript build (`tsc -b` via `npm run build`).
- Commands to run:
  - `cd crates/skilllite-assistant && npm run build`
  - `python3 scripts/validate_tasks.py`
- Manual checks (optional): with two saved profiles and routing enabled, configure an obviously bad primary key for `followup`, observe the fallback profile being used; check console / state for switch info.

## Regression Scope

- Areas likely affected: Settings modal scenario routing block, ChatView follow-up & evolution authorize handler, EvolutionSection status load + manual trigger, LifePulseBadge LLM sync.
- Explicit non-goals: Streaming chat behavior; agent loop; Rust bridge shape.

## Links

- Source TODO section: `todo/assistant-auto-llm-routing-plan.md` Phase 1 / MVP-A
- Related PRs/issues:
- Related docs: `crates/skilllite-assistant/README.md`
