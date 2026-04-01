# PRD: Unify Sub-crate Error Types

## Problem

When calling across crates, developers are unsure whether to use `anyhow::Result`,
`anyhow::bail!`, or crate-specific `XxxError` types. Inconsistent error contracts
increase review friction and maintenance cost.

## Decision

Each sub-crate defines `pub enum Error` (via `thiserror`) with:

1. Semantic variants (e.g. `Io`, `Config`, `PathValidation`)
2. `Other(#[from] anyhow::Error)` as a gradual migration escape hatch

Conventions are simplified to:
- **Inside crate X**: use `crate::Result<T>`
- **Across crates**: rely on `?` + `#[from]` auto conversion
- **CLI entrypoint**: `skilllite::Error` aggregates all sub-crate errors

## Non-Goals

- This task does not eliminate all `anyhow` usage (internal `.context()` usage remains allowed)
- Do not change success/failure modeling for non-`Result` types like `ToolResult` / `ProvisionRuntimesResult`
