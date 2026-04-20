# TASK Card

## Metadata

- Task ID: `TASK-2026-037`
- Title: Local LLM scenario routing
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Users want to assign different saved LLM profiles per call site (chat vs lightweight follow-up vs Life Pulse vs evolution bridge) without a cloud router.

## Scope

- In scope: assistant UI settings persistence; bridge `config` builders per scenario; bilingual UI strings and assistant README (ZH + EN sub-bullet).
- Out of scope: server-side routing; automatic model capability detection; Rust agent binary changes beyond existing config overlay.

## Acceptance Criteria

- [x] Toggle + per-scenario dropdowns persist in Zustand / `localStorage`.
- [x] `skilllite_chat_stream`, follow-up suggestions, Life Pulse LLM sync, and evolution-related invokes use the mapped profile when enabled.
- [x] Unmapped scenarios or invalid profile ids fall back to the main model fields.
- [x] `npm run build` passes in `crates/skilllite-assistant`.

## Risks

- Risk: User removes a saved profile that was mapped.
  - Impact: Route silently falls back to main model until remapped.
  - Mitigation: Documented behavior; optional future cleanup on save.

## Validation Plan

- Required tests: TypeScript build (`tsc -b` via `npm run build`).
- Commands to run: `cd crates/skilllite-assistant && npm run build`
- Manual checks: Enable routing, map scenarios, send chat and verify bridge uses expected profile (optional).

## Regression Scope

- Areas likely affected: Settings modal, ChatView, StatusPanel LifePulseBadge, EvolutionSection bridge config.
- Explicit non-goals: Changing default model selection when routing is off.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `crates/skilllite-assistant/README.md`
