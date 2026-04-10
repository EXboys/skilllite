# TASK Card

## Metadata

- Task ID: `TASK-2026-031`
- Title: Evolution periodic preflight and structured NoScope
- Status: `done`
- Priority: `P2`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-10`
- Target milestone:

## Problem

A9 periodic ticks often fired `run_evolution` while `build_evolution_proposals` returned empty, spamming `evolution_run_outcome` and confusing users. Empty-proposal reasons were a single generic string.

## Scope

- In scope: `GrowthDueOutcome` + periodic-only preflight in agent and desktop; structured NoScope reasons; EN/ZH docs and UI mapping; CHANGELOG.
- Out of scope: Changing default A9 intervals; altering shallow preflight semantics; policy/coordinator behavior.

## Acceptance Criteria

- [x] `growth_due` exposes whether the tick is `periodic_only` for callers.
- [x] Agent `ChatSession` skips `run_evolution` when `periodic_only` and no proposals would be built.
- [x] Desktop `evolution_growth_due` applies merged env for preflight and skips spawn under the same rule.
- [x] Empty proposals log distinct stable reason strings; assistant i18n maps them.
- [x] EN/ZH `ENV_REFERENCE.md` explains scheduling vs proposals and lists a config playbook.
- [x] Tests pass for evolution + agent; Tauri `integrations` compiles.

## Risks

- Risk: Preflight env mismatch vs subprocess
  - Impact: Wrong skip/spawn decision
  - Mitigation: Desktop uses `EvolutionRunEnvGuard::push_from_merged` for `would_have_evolution_proposals`; errors fail-open to spawn.

## Validation Plan

- Required tests: `cargo test -p skilllite-evolution -p skilllite-agent`
- Commands to run: `cargo check --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
- Manual checks: None required

## Regression Scope

- Areas likely affected: evolution growth schedule, evolution run logging, Life Pulse growth spawn, evolution panel reason display.
- Explicit non-goals: Manual `skilllite evolution run` / forced runs.

## Links

- Source TODO section: user plan attachment
- Related PRs/issues:
- Related docs: `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`, `CHANGELOG.md`
