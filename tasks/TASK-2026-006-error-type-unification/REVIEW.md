# REVIEW: TASK-2026-006 Unify Sub-crate Error Types

## Merge Readiness: ready

Merge readiness: `ready`

## Summary

Each workspace sub-crate now has a unified `pub enum Error` + `pub type Result<T>`.
Developer conventions are now simplified:

- **Inside crate X**: use `crate::Result<T>` and `crate::Error` / `bail!` macro
- **Across crates**: `?` auto-converts via `#[from]`
- **CLI entrypoint**: `skilllite::Error` aggregates all sub-crate errors

## Changes by Crate

| Crate | Error file | Added variants | Preserved legacy type |
|-------|-----------|--------------|-----------|
| skilllite-fs | Added | Io, Validation, Other | N/A |
| skilllite-core | Expanded | Io, PathValidation, Fs, Json, Yaml, Validation, Other | PathValidationError |
| skilllite-sandbox | Added | Io, BashValidation, Validation, Other | BashValidationError |
| skilllite-executor | Expanded | Io, Executor, Json, Sqlite, Validation, Other | ExecutorError |
| skilllite-evolution | Added | Io, Sqlite, Json, Http, Fs, Sandbox, Validation, Other | N/A |
| skilllite-swarm | Added | Io, Validation, Other | N/A |
| skilllite-agent | Added | Io, Json, Core, Executor, Evolution, Fs, Sandbox, Validation, Other | N/A |
| skilllite-commands | Added | Io, Json, Core, Sandbox, Fs, Evolution, Agent, Validation, Other | N/A |
| skilllite (CLI) | Expanded | +Core, +Sandbox, +Executor, +Swarm, +Agent, +Commands | PathValidation, Io, Json |

## Migration Strategy

- Keep `Other(#[from] anyhow::Error)` as a gradual migration escape hatch; internal code can still use `.context()`.
- Use crate-local `bail!` macro instead of `anyhow::bail!`.
- Preserve narrow legacy error types (`PathValidationError`, `BashValidationError`, `ExecutorError`) for backward compatibility.

## Verification

- `cargo check --workspace`: PASS
- `cargo clippy --all-targets`: PASS (0 warnings)
- `cargo test`: PASS (all tests)
- `cargo fmt --check`: PASS
