# TASK Card

## Metadata

- Task ID: `TASK-2026-005`
- Title: Sandbox: Unify Simple (Unsandboxed) Execution Path
- Status: `in_progress`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

`macos.rs` (825 lines), `linux.rs` (932 lines), and `windows.rs` (569 lines) in `skilllite-sandbox`
contain duplicated logic for:

1. Unsandboxed (simple) execution path — nearly identical across macOS and Linux.
2. Entry gate logic (check `no_sandbox` → fallback).
3. Linux has unused wrapper functions (`execute`, `execute_simple`).
4. `runner.rs` has a bug: `execute_simple_without_sandbox` for Linux calls `execute_with_limits`
   (which tries sandbox first), instead of `execute_simple_with_limits`.
5. Linux's `execute_simple_with_limits` was missing `get_script_args_from_env()` — inconsistent with macOS.

## Scope

- In scope:
  - Extract shared `execute_unsandboxed` function into `common.rs` for Unix platforms.
  - Refactor macOS and Linux `execute_simple_with_limits` to delegate to the common function.
  - Remove dead code: Linux `execute` and `execute_simple` wrappers.
  - Fix `runner.rs` `execute_simple_without_sandbox` Linux branch bug.
  - Fix Linux missing `get_script_args_from_env()` in simple execution.
  - Regression tests.
- Out of scope:
  - Refactoring platform-specific sandbox implementations (Seatbelt, bwrap, firejail, WSL2).
  - Changing Windows execution model (differs significantly from Unix).
  - Trait-based sandbox dispatch redesign (already has `SandboxBackend` trait).

## Acceptance Criteria

- [x] Shared `execute_unsandboxed` function exists in `common.rs` and is used by macOS and Linux.
- [x] Linux dead wrappers (`execute`, `execute_simple`) removed.
- [x] `runner.rs` Linux branch correctly calls `execute_simple_with_limits`.
- [x] Linux simple execution now includes `get_script_args_from_env()` (consistency fix).
- [x] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass.

## Risks

- Risk: Behavioral difference between macOS and Linux simple execution paths.
  - Impact: Execution failure on one platform.
  - Mitigation: Unified function ensures identical behavior; verified by tests.

## Validation Plan

- Required tests:
  - `cargo fmt --check`
  - `cargo clippy --all-targets`
  - `cargo test`
  - `cargo test -p skilllite-sandbox`
- Manual checks:
  - Verify macOS and Linux `execute_simple_with_limits` delegate correctly.
  - Verify runner.rs dispatch is correct for all platforms.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-sandbox/src/common.rs`
  - `crates/skilllite-sandbox/src/macos.rs`
  - `crates/skilllite-sandbox/src/linux.rs`
  - `crates/skilllite-sandbox/src/runner.rs`
- Explicit non-goals:
  - Sandbox policy/security behavior.
  - Windows execution path (unchanged).

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (§0.2 #6 Platform sandbox code duplication)
- Related PRs/issues:
- Related docs: N/A
