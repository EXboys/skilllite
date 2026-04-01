# PRD — Platform Sandbox Dedup

## What

Extract duplicated unsandboxed execution logic from macOS and Linux into a shared
function in `common.rs`. Fix inconsistencies and dead code.

## Why

- macOS and Linux `execute_simple_with_limits` are near-identical (~35 lines each).
- Linux was missing `get_script_args_from_env()` — behavioral inconsistency.
- `runner.rs` had a bug: Linux `execute_simple_without_sandbox` incorrectly called
  `execute_with_limits` (which attempts sandbox first) instead of `execute_simple_with_limits`.
- Linux had dead wrapper functions (`execute`, `execute_simple`) adding noise.

## Design Decisions

1. **Unix-only shared function** (`#[cfg(unix)]`): Windows execution model differs too much
   (Job Object instead of rlimits, manual wait loop, input file instead of stdin pipe).
2. **Relative entry point path**: Unified to use `config.entry_point` (relative to `skill_dir`),
   since `current_dir` is always set to `skill_dir`. Linux previously used absolute path unnecessarily.
3. **No trait refactoring**: The existing `SandboxBackend` trait already handles dispatch.
   Over-abstracting the entry gate logic would add complexity without clear benefit.
