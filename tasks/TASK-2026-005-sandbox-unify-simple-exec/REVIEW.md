# REVIEW — TASK-2026-005

## Summary

Extracted duplicated unsandboxed execution logic from macOS and Linux platform sandbox
files into a shared `common::execute_unsandboxed` function. Fixed a bug in `runner.rs`
where the Linux no-sandbox path incorrectly attempted sandbox execution. Removed dead code.

## Findings

### Bug Fixed
- `runner.rs` `execute_simple_without_sandbox` for Linux was calling `linux::execute_with_limits`
  (which attempts sandbox first and may fail/fallback) instead of `linux::execute_simple_with_limits`
  (which runs without any sandbox). This means Level 1 (no sandbox) on Linux was not truly
  bypassing the sandbox.

### Consistency Fix
- Linux `execute_simple_with_limits` was missing `get_script_args_from_env()` call that macOS had.
  The unified function now includes it, making behavior consistent across platforms.

### Dead Code Removed
- Linux `execute` (pub, unused wrapper around `execute_with_limits`)
- Linux `execute_simple` (private, unused wrapper around `execute_simple_with_limits`)
- `runner.rs` `execute_platform_sandbox` (3 platform variants, never called — only the
  `_with_limits` versions were used)

## Merge Readiness

- [x] All acceptance criteria met
- [x] No behavioral regression (65 sandbox tests pass)
- [x] Bug fix verified (runner.rs now calls correct function)
- [x] Format and lint clean

Merge readiness: `ready`

## Verification Evidence

```
$ cargo fmt --check    → exit 0
$ cargo clippy --all-targets → exit 0, no errors
$ cargo test -p skilllite-sandbox → 65 passed, 0 failed
$ cargo test → all workspace tests pass
```
