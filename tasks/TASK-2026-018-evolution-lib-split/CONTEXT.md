# CONTEXT

## Layout (after change)

- `llm.rs` — `EvolutionMessage`, `EvolutionLlm`, `strip_think_blocks` (closing tags as inline `for` array; opening tags const slice).
- `config.rs` — `EvolutionMode`, `SkillAction`, `EvolutionThresholds`, `EvolutionProfile`.
- `run_state.rs` — evolution mutex and `EvolutionRunResult`.
- `scope.rs` — `EvolutionScope`, proposals, coordinator, `should_evolve*`, backlog helpers; several `pub(crate)` items for `run` and tests.
- `gatekeeper.rs`, `snapshots.rs`, `changelog.rs`, `audit.rs`, `rollback.rs`, `run.rs`, `lifecycle.rs` — names match responsibilities.
- `lib.rs` — `pub mod` + `pub use` barrel and `#[cfg(test)] mod lib_tests`.

## Tag spelling fix

Some test sources had been corrupted to `<redacted_thinking>` / `</redacted_thinking>` instead of `<redacted_thinking>` / `</redacted_thinking>`, which broke `strip_think_blocks` expectations. Corrected in `llm.rs`, `skill_synth/parse.rs`, and `prompt_learner.rs` where needed.

## Dependencies

- `run.rs` imports `rollback`, `scope`, `snapshots`, `changelog`, `audit`, `feedback`, learners, `skill_synth`, `external_learner`.
- Submodules continue to use `crate::strip_think_blocks` and `crate::gatekeeper_*` via `lib` re-exports.
