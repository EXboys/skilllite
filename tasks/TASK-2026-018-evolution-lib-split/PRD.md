# PRD

## Goal

Improve maintainability of the evolution crate by splitting the monolithic `lib.rs` into functional modules without changing observable behavior or public paths.

## Requirements

1. New modules under `crates/skilllite-evolution/src/` own coherent responsibilities (LLM surface, env config, scope/coordinator, gatekeeper, snapshots, changelog, audit, rollback, run loop, shutdown hook).
2. `lib.rs` re-exports the same symbols as before so `use skilllite_evolution::...` remains stable.
3. No intentional behavior change; tests must pass.

## Non-goals

- Redesigning evolution algorithms or database schema.
- Publishing new public types beyond what was already public (except re-export clarity for `EvolutionProfile` at crate root, already public in the previous `lib.rs`).

## Verification

- Automated tests and clippy as listed in `TASK.md`.
