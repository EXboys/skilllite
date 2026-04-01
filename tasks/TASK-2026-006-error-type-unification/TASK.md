# TASK Card

## Metadata

- Task ID: `TASK-2026-006`
- Title: Unify Sub-crate Error Types
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

The CLI boundary already uses `skilllite::Error` + `thiserror`, but workspace sub-crates still rely heavily on `anyhow`.
Cross-crate error propagation lacks a consistent contract, increasing review and maintenance overhead.

## Scope

- In scope:
  - Define a crate-level `Error` enum + `pub type Result<T>` for each sub-crate.
  - Add `#[from]` conversions from sub-crate errors into CLI `skilllite::Error`.
  - Migrate public API signatures from `anyhow::Result` to `crate::Result`.
- Out of scope:
  - Removing all internal `anyhow` usage in one pass (keep `Other(#[from] anyhow::Error)` as gradual migration escape hatch).

## Acceptance Criteria

- [x] Each sub-crate has exactly one `pub enum Error` and one `pub type Result<T>`
- [x] Public APIs (exported from `lib.rs`) return `crate::Result<T>`
- [x] CLI crate `Error` includes `#[from]` auto-conversions for sub-crate errors
- [x] `cargo check --workspace` passes
- [x] `cargo clippy --all-targets` has no new warnings
- [x] `cargo test` passes for the full workspace
- [x] Existing narrow error enums (`PathValidationError`, `BashValidationError`) are preserved as variants

## Risks

- Large-scale signature changes may cause compile breakage; migration must proceed bottom-up per crate.
- `#[from] anyhow::Error` can produce conflicting impls in nested conversions; care is required.

## Validation Plan

- Required tests:
  - `cargo check --workspace`
  - `cargo clippy --all-targets`
  - `cargo test`
- Manual checks:
  - Verify cross-crate `?` conversion paths compile successfully.

## Regression Scope

- Areas likely affected:
  - Cross-crate error propagation paths and CLI aggregated error display.
- Explicit non-goals:
  - Rewriting business-facing error text.
