# STATUS: TASK-2026-006 Unify Sub-crate Error Types

## Current Status: done

## Timeline

- 2026-04-01:
  - Progress:
    - Completed unified `Error`/`Result` migration across sub-crates.
    - Completed CLI aggregated `#[from]` error integration.
  - Blockers:
    - None.
  - Next step:
    - Continue refining domain-specific error semantics during maintenance.

## Checkpoints

### 2026-04-01: Full rollout completed

- [x] `skilllite-fs`: Added `Error` + `Result<T>` + `bail!` macro
- [x] `skilllite-core`: Preserved `PathValidationError`, added crate-level `Error` (Json/Yaml/Fs variants)
- [x] `skilllite-sandbox`: Preserved `BashValidationError`, added crate-level `Error` + `bail!` macro
- [x] `skilllite-executor`: Preserved `ExecutorError`, added crate-level `Error` (Sqlite variants)
- [x] `skilllite-evolution`: Added `Error` (Sqlite/Json/Http/Fs/Sandbox variants)
- [x] `skilllite-swarm`: Added `Error`
- [x] `skilllite-agent`: Added `Error` (Core/Executor/Evolution/Fs/Sandbox variants)
- [x] `skilllite-commands`: Added `Error` (Core/Sandbox/Fs/Evolution/Agent variants)
- [x] `skilllite` CLI `Error`: Added 6 `#[from]` variants for Core/Sandbox/Executor/Swarm/Agent/Commands
- [x] `cargo check --workspace` passed
- [x] `cargo clippy --all-targets` passed with zero warnings
- [x] `cargo test` passed for full workspace
- [x] `cargo fmt --check` passed
