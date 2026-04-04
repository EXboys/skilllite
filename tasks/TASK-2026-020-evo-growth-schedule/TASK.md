# TASK Card

## Metadata

- Task ID: `TASK-2026-020`
- Title: Structured evolution growth schedule (A9)
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Created: `2026-04-04`

## Scope

- In scope: `growth_schedule` module, env keys, agent + desktop integration, docs, UI status fields, tests.
- Out of scope: two-phase LLM evolution planning inside learners; new token accounting.

## Acceptance Criteria

- [x] Shared A9 due logic in `skilllite-evolution` with weighted window, sweep, min gap, OR raw threshold.
- [x] Default periodic interval 600s when env unset; active stable minimum decoupled via `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS`.
- [x] Life Pulse + `ChatSession` use the same rules (merged overrides on desktop for interval/raw threshold).
- [x] `ENV_REFERENCE` EN/ZH + `CHANGELOG` updated; evolution panel shows weighted fields.
- [x] `cargo test -p skilllite-evolution -p skilllite-agent`; `cargo check` Tauri; `cargo clippy` evolution + agent.

## Validation (evidence)

- `cargo test -p skilllite-evolution growth_schedule` — pass.
- `cargo test -p skilllite-agent` — 190 tests pass.
- `cargo clippy -p skilllite-evolution -p skilllite-agent --all-targets` — clean.
- `cargo check` in `crates/skilllite-assistant/src-tauri` — pass.

## Regression Scope

- Evolution autorun timing and frequency (defaults more aggressive on weighted arm).
- Active proposal gating if operators relied on `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` for both behaviors.

## Links

- PRD: `PRD.md`
- Context: `CONTEXT.md`
