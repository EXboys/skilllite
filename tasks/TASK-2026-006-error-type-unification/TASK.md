# TASK-2026-006: 子 crate 错误类型统一

## Summary

主 CLI 边界已有 `skilllite::Error` + `thiserror`，但 workspace 子 crate 仍普遍使用 `anyhow`。
跨层错误传播时"该用哪种 Error"认知负担较高。统一每个子 crate 的错误类型。

## Owner

exboys

## Priority

P1

## Scope

为每个子 crate 定义 crate-level `Error` enum + `pub type Result<T>`:

1. **skilllite-fs** — 新建 `error.rs`
2. **skilllite-core** — 提升 `PathValidationError` → crate-level `Error`
3. **skilllite-sandbox** — 提升 `BashValidationError` → crate-level `Error`
4. **skilllite-executor** — 提升 `ExecutorError` → crate-level `Error`
5. **skilllite-evolution** — 新建 `error.rs`
6. **skilllite-swarm** — 新建 `error.rs`
7. **skilllite-agent** — 新建 `error.rs`
8. **skilllite-commands** — 新建 `error.rs`
9. **skilllite CLI** — Error enum 添加 `#[from]` sub-crate variants

每个 crate Error 保留 `Other(#[from] anyhow::Error)` 作为渐进迁移逃生口。
公开 API 签名从 `anyhow::Result` → `crate::Result`。

## Acceptance Criteria

- [x] 每个子 crate 有且仅有一个 `pub enum Error` 和 `pub type Result<T>`
- [x] 公开 API（lib.rs 导出的函数）返回 `crate::Result<T>`
- [x] CLI crate Error 对子 crate Error 有 `#[from]` 自动转换
- [x] `cargo check --workspace` 通过
- [x] `cargo clippy --all-targets` 无新增警告
- [x] `cargo test` 全量通过
- [x] 已有的特定错误枚举（PathValidationError, BashValidationError）保留为 Error variant

## Risks

- 大量函数签名变更可能导致编译断裂，需自下而上逐 crate 推进
- `#[from] anyhow::Error` 在多层嵌套时可能产生 conflicting impl，需注意

## Regression Scope

- 全量 `cargo test`
- 特别关注 E2E 测试路径
