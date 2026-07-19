# Technical Context

## Current State

- Relevant crates/files: `skilllite/src/stdio_rpc_params.rs`,
  `skilllite/src/stdio_rpc.rs`, and
  `crates/skilllite-sandbox/src/runner.rs`.
- Current behavior: `IpcRunParams` and `IpcExecParams` parse a JSON `u64` and cast
  it with `as u8`. `handle_run` and `handle_exec` then pass that value to
  `SandboxLevel::from_env_or_cli`; a truncated `1` selects direct unsandboxed
  execution.

## Architecture Fit

- Layer boundaries involved: Entry-layer stdio parsing calls command execution
  with a sandbox-layer enum. The dependency direction remains unchanged.
- Interfaces to preserve: JSON-RPC method names and response framing,
  `IpcRunParams`/`IpcExecParams` field types, and
  `SandboxLevel::from_env_or_cli`.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Documented values 1 through 3 and omission are
  compatible. Invalid values become explicit errors instead of silently weakening
  or defaulting security.

## Design Decisions

- Decision: Add one shared `opt_sandbox_level` parser and use it for both stdio
  execution methods.
  - Rationale: Validation before narrowing conversion removes truncation and keeps
    both security-sensitive call sites consistent.
  - Alternatives considered: Use `u8::try_from` only, validate in the Python SDK,
    or let `SandboxLevel::from_env_or_cli` handle invalid values.
  - Why rejected: `u8::try_from` still accepts levels 4 through 255; Python-only
    validation leaves raw JSON-RPC vulnerable; downstream validation cannot
    recover the original value after truncation.

## Open Questions

- [x] Should malformed values fall back to level 3? No. The MCP path establishes
  explicit validation, and rejecting malformed security input is fail closed.
- [x] Are docs updates needed? No. The documented contract already permits only
  levels 1 through 3; this change enforces that contract.
