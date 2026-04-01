# REVIEW: TASK-2026-006 子 crate 错误类型统一

## Merge Readiness: ready

## Summary

每个 workspace 子 crate 现在都有统一的 `pub enum Error` + `pub type Result<T>`。
开发者认知规则简化为：

- **在 crate X 内**：使用 `crate::Result<T>` 和 `crate::Error` / `bail!` 宏
- **跨 crate 调用**：`?` 通过 `#[from]` 自动转换
- **CLI 入口**：`skilllite::Error` 聚合所有子 crate Error

## Changes by Crate

| Crate | Error 文件 | 新增 Variants | 保留旧类型 |
|-------|-----------|--------------|-----------|
| skilllite-fs | 新建 | Io, Validation, Other | N/A |
| skilllite-core | 扩展 | Io, PathValidation, Fs, Json, Yaml, Validation, Other | PathValidationError |
| skilllite-sandbox | 新建 | Io, BashValidation, Validation, Other | BashValidationError |
| skilllite-executor | 扩展 | Io, Executor, Json, Sqlite, Validation, Other | ExecutorError |
| skilllite-evolution | 新建 | Io, Sqlite, Json, Http, Fs, Sandbox, Validation, Other | N/A |
| skilllite-swarm | 新建 | Io, Validation, Other | N/A |
| skilllite-agent | 新建 | Io, Json, Core, Executor, Evolution, Fs, Sandbox, Validation, Other | N/A |
| skilllite-commands | 新建 | Io, Json, Core, Sandbox, Fs, Evolution, Agent, Validation, Other | N/A |
| skilllite (CLI) | 扩展 | +Core, +Sandbox, +Executor, +Swarm, +Agent, +Commands | PathValidation, Io, Json |

## Migration Strategy

- `Other(#[from] anyhow::Error)` 作为渐进迁移逃生口，内部代码仍可用 `.context()` 等 anyhow 特性
- `bail!` crate-local 宏替代 `anyhow::bail!`
- 已有窄错误类型（PathValidationError, BashValidationError, ExecutorError）保持 backward-compatible

## Verification

- `cargo check --workspace`: PASS
- `cargo clippy --all-targets`: PASS (0 warnings)
- `cargo test`: PASS (all tests)
- `cargo fmt --check`: PASS
