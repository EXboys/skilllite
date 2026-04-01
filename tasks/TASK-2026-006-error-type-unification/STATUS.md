# STATUS: TASK-2026-006 子 crate 错误类型统一

## Current Status: done

## Checkpoints

### 2026-04-01: 全量实施完成

- [x] skilllite-fs: 新建 `Error` + `Result<T>` + `bail!` 宏
- [x] skilllite-core: `PathValidationError` 保留，新增 crate-level `Error` (含 Json/Yaml/Fs 变体)
- [x] skilllite-sandbox: `BashValidationError` 保留，新增 crate-level `Error` + `bail!` 宏
- [x] skilllite-executor: `ExecutorError` 保留，新增 crate-level `Error` (含 Sqlite 变体)
- [x] skilllite-evolution: 新建 `Error` (含 Sqlite/Json/Http/Fs/Sandbox 变体)
- [x] skilllite-swarm: 新建 `Error`
- [x] skilllite-agent: 新建 `Error` (含 Core/Executor/Evolution/Fs/Sandbox 变体)
- [x] skilllite-commands: 新建 `Error` (含 Core/Sandbox/Fs/Evolution/Agent 变体)
- [x] skilllite CLI `Error`: 新增 Core/Sandbox/Executor/Swarm/Agent/Commands 6 个 `#[from]` variants
- [x] `cargo check --workspace` 通过
- [x] `cargo clippy --all-targets` 零 warning
- [x] `cargo test` 全量通过
- [x] `cargo fmt --check` 通过
