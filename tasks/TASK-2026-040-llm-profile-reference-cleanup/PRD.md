# PRD

## Background

Routing state now stores primary and fallback references to saved profiles. Without cleanup, deleting or losing those profiles leaves the settings in a partially broken state that only recovers at runtime.

## Objective

Keep routing references aligned with the actual saved profile list so configuration remains trustworthy and self-healing.

## Functional Requirements

- FR-1: Missing profile ids are removed from routing state automatically.
- FR-2: Profile deletion updates primary and fallback references consistently.
- FR-3: The settings UI reflects the cleaned state immediately and does not retain ghost chips/select values.

## Non-Functional Requirements

- Security: Cleanup must not infer provider validity from network calls.
- Performance: Cleanup should happen in memory without noticeable UI cost.
- Compatibility: Older persisted state with stale ids should recover on load.

## Constraints

- Technical: Assistant TypeScript only unless a deeper persistence migration is needed.
- Timeline: Focused follow-up after MVP-A.

## Success Metrics

- Metric: No stale profile ids remain in persisted routing references after cleanup points run.
- Baseline: Runtime ignores missing ids but state may still contain them.
- Target: Deterministic cleanup and correct UI reflection.

## Rollout

- Rollout plan: Add cleanup helper(s), call them at deletion/save/load boundaries, then verify persisted state shape.
- Rollback plan: Disable aggressive cleanup and keep the current runtime fallback behavior if an edge case appears.
