# PRD

## Background

Scenario routing already maps each call site to a saved profile. Without fallback, a transient provider error (rate limit, brief 5xx, timeout, network blip) propagates straight to the UI as a failed call. MVP-A from the routing plan adds **runtime fallback for non-streaming scenarios** so users get a more reliable experience without introducing cloud control or smart classification.

## Objective

When a non-streaming scenario invoke fails with a retryable error, transparently retry against the configured ordered fallback list, with a short in-memory cooldown for failed profiles.

## Functional Requirements

- FR-1: Each scenario can store an ordered fallback list of saved profile ids in `Settings`.
- FR-2: A shared helper builds candidate chain `[primary, ...fallbacks]`, skips cooling-down profiles, and tries each in order.
- FR-3: Retryable errors (429 / 5xx / timeout / common network strings) trigger switching; other errors propagate immediately.
- FR-4: Wiring covers `followup`, `lifePulse`, and the two evolution non-streaming invokes (status load + manual trigger).
- FR-5: Settings UI exposes both the primary mapping and the fallback list per scenario.

## Non-Functional Requirements

- Security: No new credential storage paths; reuses existing `llmProfiles` persisted in `localStorage`.
- Performance: Cooldown is an in-process `Map`; no extra round trips on the happy path.
- Compatibility: Old persisted state without the new key behaves as “no fallbacks configured”.

## Constraints

- Technical: Implementation lives in assistant TypeScript only; Rust bridge contract unchanged.
- Timeline: Single iteration as MVP-A.

## Success Metrics

- Metric: Build passes; non-streaming invokes go through the fallback helper.
- Baseline: N/A.
- Target: Green `npm run build`; manual error injection triggers the second profile.

## Rollout

- Rollout plan: Ship with assistant; users opt in by adding fallback profiles in Settings.
- Rollback plan: Clear fallback list (or disable scenario routing) and behavior matches the previous version.
