# PRD

## Background

SkillLite Assistant already persists multiple LLM profiles and builds a single `ChatConfigOverrides` object for Tauri bridge calls. A lightweight local router lets users steer different features to cheaper or stronger models without remote policy.

## Objective

Ship a local-only, rule-based mapping from fixed scenarios to saved profile IDs, applied when building bridge config.

## Functional Requirements

- FR-1: User can enable routing and assign zero or one saved profile per scenario (`agent`, `followup`, `lifePulse`, `evolution`).
- FR-2: When disabled or when a scenario has no mapping, behavior matches the previous single global model fields.

## Non-Functional Requirements

- Security: No new network surface; credentials remain in existing persisted settings.
- Performance: Negligible (object copy + lookup).
- Compatibility: Backward compatible; new fields optional.

## Constraints

- Technical: Implemented in assistant TypeScript only; Rust bridge shape unchanged.
- Timeline: Single iteration.

## Success Metrics

- Metric: Build passes; routing applies to all listed invoke sites.
- Baseline: N/A.
- Target: Green `npm run build`.

## Rollout

- Rollout plan: Ship with assistant; users opt in via Settings.
- Rollback plan: Disable toggle or clear mappings.
