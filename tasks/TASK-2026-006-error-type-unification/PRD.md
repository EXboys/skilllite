# PRD: 子 crate 错误类型统一

## Problem

跨 crate 调用时，开发者不确定该用 `anyhow::Result`、`anyhow::bail!`、
还是某个 crate 的特定 `XxxError`。错误类型不一致增加代码审查和维护成本。

## Decision

每个子 crate 定义 `pub enum Error`（thiserror），包含:

1. 语义化 variants（如 `Io`, `Config`, `PathValidation` 等）
2. `Other(#[from] anyhow::Error)` 作为渐进迁移逃生口

规则简化为：
- **在 crate X 内**：使用 `crate::Result<T>`
- **跨 crate 调用**：`?` 自动通过 `#[from]` 转换
- **CLI 入口**：`skilllite::Error` 聚合所有子 crate Error

## Non-Goals

- 本任务不消除所有 `anyhow` 使用（内部实现仍可通过 `.context()` 等使用 anyhow）
- 不改变 `ToolResult` / `ProvisionRuntimesResult` 等非 `Result` 的成功/失败模型
