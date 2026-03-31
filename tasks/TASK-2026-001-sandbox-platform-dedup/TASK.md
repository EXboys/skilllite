# TASK Card

## Metadata

- Task ID: `TASK-2026-001`
- Title: Deduplicate sandbox platform logic (macOS/Linux/Windows)
- Status: `ready`
- Priority: `P1`
- Owner: `TBD`
- Contributors: `TBD`
- Created: `2026-03-31`
- Target milestone: `v0.1.x maintenance`

## Problem

Platform sandbox files contain repeated resource-limit, logging, and error-handling flows, increasing review and regression cost.

## Scope

- In scope:
  - Extract shared sandbox execution flow into common abstractions.
  - Keep platform-specific isolation implementation in dedicated modules.
  - Preserve existing security semantics and fail-closed behavior.
- Out of scope:
  - Changing default sandbox level semantics.
  - Relaxing any security restrictions.

## Acceptance Criteria

- [ ] Shared flow is centralized and reused by all three platform backends.
- [ ] Existing behavior for level gating and fallback remains unchanged.
- [ ] Regression tests pass for sandbox crate and e2e minimal path.

## Risks

- Risk: subtle behavior drift per platform.
  - Impact: security regressions or runtime breakage.
  - Mitigation: parity checklist plus targeted tests per backend path.

## Validation Plan

- Required tests: sandbox unit/integration + minimal e2e.
- Commands to run:
  - `cargo test -p skilllite-sandbox`
  - `cargo test -p skilllite`
  - `cargo test --test e2e_minimal -p skilllite`
- Manual checks:
  - Verify fail-closed behavior on Linux fallback path.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-sandbox/src/macos.rs`
  - `crates/skilllite-sandbox/src/linux.rs`
  - `crates/skilllite-sandbox/src/windows.rs`
- Explicit non-goals:
  - No new policy permissions.

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (`0.2 #6`, `0.2 large-file refactor`)
- Related PRs/issues: `TBD`
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
