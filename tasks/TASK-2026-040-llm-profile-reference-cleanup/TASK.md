# TASK Card

## Metadata

- Task ID: `TASK-2026-040`
- Title: LLM routing profile reference cleanup
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Scenario routing and fallback lists can reference saved profiles that later get deleted or otherwise become stale. The runtime currently falls back defensively, but the settings state can still drift into a confusing "configured but not real" condition.

## Scope

- In scope:
  - Detect stale `llmScenarioRoutes` and `llmScenarioFallbacks` references.
  - Decide where cleanup happens (on save, on profile delete, on settings load, or a combination).
  - Ensure deleting a profile removes or repairs any primary/fallback references to it.
  - Surface minimal user feedback when an invalid reference is auto-cleaned.
- Out of scope:
  - Runtime health checks against provider credentials.
  - Smart model capability validation.
  - Cross-device config sync.

## Acceptance Criteria

- [x] Deleting a saved profile removes it from all scenario primary/fallback references.
- [x] Loading settings with stale routing references cleans them deterministically.
- [x] The UI does not continue to display ghost profile references after cleanup.
- [x] Validation evidence covers at least one stale-primary and one stale-fallback case.

## Risks

- Risk: Over-eager cleanup could remove references the user expected to keep after an edit.
  - Impact: Unexpected configuration loss.
  - Mitigation: Keep cleanup deterministic, narrowly scoped to missing ids, and surface a subtle notice if needed.

## Validation Plan

- Required tests: assistant build + focused logic verification for the helper.
- Commands to run:
  - `cd crates/skilllite-assistant && npm run build`
  - one-off Node+TypeScript transpile script verifying stale-primary / stale-fallback cleanup
  - `python3 scripts/validate_tasks.py`
- Manual checks: delete a profile currently used by primary/fallback routing and verify the settings block updates correctly.

## Regression Scope

- Areas likely affected: `llmProfiles` helpers, settings persistence/load, Settings UI scenario routing block, quick-switch delete path.
- Explicit non-goals: network-level validity of provider keys.

## Links

- Source TODO section: `todo/assistant-auto-llm-routing-plan.md` §10.2
- Related PRs/issues:
- Related docs: `crates/skilllite-assistant/README.md`, `todo/assistant-auto-llm-routing-plan.md`
