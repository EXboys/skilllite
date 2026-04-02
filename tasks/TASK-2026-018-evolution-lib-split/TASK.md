# TASK Card

## Metadata

- Task ID: `TASK-2026-018`
- Title: Split skilllite-evolution lib.rs into modules
- Status: `done`
- Priority: `P2`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-02`
- Target milestone:

## Problem

`crates/skilllite-evolution/src/lib.rs` had grown to ~2900 lines, mixing LLM integration, configuration, scope/coordinator logic, gatekeepers, snapshots, changelog, audit, run orchestration, lifecycle, and rollback. This hurt navigation and review.

## Scope

- In scope: Mechanical split into focused modules under `src/`; preserve public API via `pub use` from `lib.rs`; keep behavior identical; fix any test strings corrupted to wrong think-tag spellings discovered during validation.
- Out of scope: Further splitting `scope.rs`; changing evolution behavior; documentation rewrites outside task artifacts.

## Acceptance Criteria

- [x] `lib.rs` reduced to crate root: module declarations, re-exports, and unit tests.
- [x] Logic grouped into named modules (`llm`, `config`, `run_state`, `scope`, `gatekeeper`, `snapshots`, `changelog`, `audit`, `rollback`, `run`, `lifecycle`).
- [x] Downstream crates (`skilllite-agent`, `skilllite-commands`) still compile against existing `skilllite_evolution::...` paths.
- [x] `cargo test -p skilllite-evolution` passes; `cargo clippy -p skilllite-evolution --all-targets` passes.

## Risks

- Risk: Missed `pub` re-export breaks external callers.
  - Impact: Compile errors in workspace.
  - Mitigation: `cargo check` on dependent crates; grep `skilllite_evolution::` usage.

- Risk: Subtle visibility / test-only API drift.
  - Impact: Failing or vacuous tests.
  - Mitigation: Run full evolution lib tests; use `pub(crate)` only where tests and `run` need shared helpers.

## Validation Plan

- Required tests: `skilllite-evolution` lib tests (including `lib_tests` and `skill_synth::parse` think-block tests).
- Commands to run: `cargo test -p skilllite-evolution`, `cargo clippy -p skilllite-evolution --all-targets`, `cargo check -p skilllite-agent -p skilllite-commands`.
- Manual checks: None.

## Regression Scope

- Areas likely affected: `skilllite-evolution` crate layout and any code depending on `strip_think_blocks` / gatekeeper paths (unchanged at crate root).
- Explicit non-goals: Performance tuning; API redesign.

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (evolution lib size).
- Related PRs/issues:
- Related docs:
