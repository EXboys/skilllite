# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-core/src/skill/discovery.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/paths.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/bundled_skills_sync.rs`
  - `crates/skilllite-evolution/src/skill_synth/mod.rs`
- Current behavior:
  - Core already owns the canonical workspace skill search directories and legacy fallback resolution.
  - Assistant duplicates skill enumeration for UI actions and hardcodes `.skills` for some pending/evolution flows.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-assistant` may depend on `skilllite-core`, but `skilllite-core` must remain lower-layer and generic.
- Interfaces to preserve:
  - Existing assistant Tauri commands and evolution APIs should keep their public signatures.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Keep optional legacy fallback from `skills/` to `.skills/`.
  - Do not break workspaces that still store evolved/pending skills under the resolved canonical skill root.

## Design Decisions

- Decision: Move assistant-visible skill instance enumeration into `skilllite_core::skill::discovery`.
  - Rationale: The supported skill root set and fallback rules already live in core and should not be re-encoded in assistant.
  - Alternatives considered: Keep the helper inside assistant but call `SKILL_SEARCH_DIRS`.
  - Why rejected: That still leaves assistant responsible for root semantics and evolved traversal ordering.
- Decision: Keep assistant-specific filtering such as "has scripts" in assistant.
  - Rationale: Script presence is a UI/business rule, not a generic core discovery invariant.
  - Alternatives considered: Move script filtering into core.
  - Why rejected: It would overfit core discovery to one assistant workflow.

## Open Questions

- [x] Assistant precedence remains deterministic by path-sorted discovery output plus name deduplication in the UI-facing list; no extra override rule was required in this task.
- [x] Root docs were updated in `README.md` and `docs/zh/README.md` because the supported discovery roots are user-visible behavior.
