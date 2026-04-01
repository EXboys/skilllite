# CONTEXT — Platform Sandbox Dedup

## Technical Boundaries

- `common.rs` already contains platform-specific `#[cfg]` blocks (e.g. `get_process_memory`,
  `resolve_which`), so adding Unix-shared logic there is architecturally consistent.
- `SandboxBackend` trait in `sandbox_backend.rs` dispatches to platform `execute_with_limits`.
  This task does not change that dispatch path.
- `runner.rs` `execute_platform_sandbox_with_limits` and `execute_simple_without_sandbox`
  are the two caller paths into platform code. Both must remain correct after refactoring.

## Constraints

- macOS `execute_simple_with_limits` is `pub fn` — called from `runner.rs` Level 1 path.
  Must keep the same signature and remain public.
- Linux `execute_simple_with_limits` is `fn` (private) — only called internally from
  `execute_with_limits`. Can remain private but needs the common delegation.
- Windows `execute_simple_with_limits` is `pub fn` — not changed in this task.

## Compatibility

- No public API changes.
- No CLI/MCP behavior changes.
- No environment variable changes.
