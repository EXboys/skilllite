# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Advance Life Pulse periodic growth anchor
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-09-04`
- Target milestone:

## Problem

Desktop Life Pulse never advances `last_periodic_growth_unix`. `evolution_growth_due` only reads the mutex and calls inspect-only `skilllite evolution status --json`. `inspect_growth_due` treats a missing anchor as `now`, so `arm_periodic` stays false on every heartbeat. Trigger: enable Life Pulse with high signal/sweep thresholds so only the periodic arm can fire; wait past `SKILLLITE_EVOLUTION_INTERVAL_SECS`. Expected: spawn `skilllite evolution run`. Actual: no periodic spawn. Chat/agent-rpc still works because it uses mutating `growth_due`.

## Scope

- In scope:
  - Advance the Life Pulse periodic mutex the same way `growth_due` mutates `last_periodic_spawn_unix` (seed `None` to now; refresh when `arm_periodic`).
  - Keep the L2 CLI-only status path (no `skilllite-evolution` dependency in the assistant crate).
  - Unit tests for anchor advancement and spawn gating.
- Out of scope:
  - Life Pulse rhythm `--workspace` (#115/#116).
  - Evolution reset/repair workspace roots (#113/#120).
  - ChatSession `config.workspace` split (#114).
  - Broad Life Pulse refactors.

## Acceptance Criteria

- [x] First heartbeat with a missing anchor persists `now` so later inspects can accumulate elapsed time.
- [x] When status reports `arm_periodic`, the mutex refreshes to `now` (including periodic-only ticks skipped because no proposals).
- [x] Signal/sweep-only dues do not reset the periodic anchor.
- [x] Spawn gating stays: disabled / not due / periodic-only-without-proposals still skip.
- [x] Regression tests fail if the mutex is left at `None` or is not refreshed on `arm_periodic`.

## Risks

- Risk: Advancing the anchor on a skipped no-proposal tick delays the next periodic attempt by a full interval.
  - Impact: Matches in-process `growth_due` + `would_have_evolution_proposals` skip in `chat_session`.
  - Mitigation: Mirror that contract explicitly in tests.
- Risk: False positive that signal/sweep growth is also broken.
  - Impact: Unnecessary extra changes.
  - Mitigation: Only persist the periodic mutex; do not change signal/sweep CLI status fields.

## Validation Plan

- Required tests: Targeted assistant unit tests for `next_periodic_anchor` and `should_spawn_growth`; workspace clippy/tests.
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml next_periodic_anchor should_spawn_growth`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks:
  - Trace Life Pulse heartbeat → `evolution_growth_due` → mutex write → later `inspect_growth_due` elapsed.
  - Confirm Slack/PR output matches the final outcome.

## Regression Scope

- Areas likely affected: Desktop Life Pulse periodic growth spawn cadence.
- Explicit non-goals: Rhythm `schedule tick` workspace, destructive evolution commands, ChatSession workspace alignment.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-09-04.
- Related PRs/issues: Open drafts #115/#116 (rhythm workspace); this is a distinct periodic-anchor write-back gap.
- Related docs: `docs/en/ENV_REFERENCE.md` A9 periodic arm; `spec/verification-integrity.md`, `spec/task-artifact-language.md`.
