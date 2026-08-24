# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/builtin/mod.rs` (`execute_builtin_tool`)
  - `crates/skilllite-agent/src/extensions/builtin/helpers.rs` (`parse_truncated_json_for_file_tools`)
  - `crates/skilllite-agent/src/extensions/builtin/file_ops/mod.rs` (`execute_write_file`)
  - `crates/skilllite-agent/src/extensions/builtin/output.rs` (`execute_write_output`)
  - `crates/skilllite-agent/src/extensions/registry.rs` (schema skip for these two tools)
- Current behavior:
  - Invalid JSON for `write_file` / `write_output` is recovered via regex.
  - `"content": "` with no payload becomes `content: ""`.
  - `execute_write_file` / `execute_write_output` then overwrite the resolved path.

## Architecture Fit

- Layer boundaries involved: builtin tool dispatch inside `skilllite-agent` only.
- Interfaces to preserve: `ToolResult` shape, valid JSON write semantics, non-empty recovery + warning.

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: only the invalid-JSON empty-content recovery path becomes an error. Callers that send complete JSON are unchanged.

## Design Decisions

- Decision: reject empty/missing recovered `content` in `execute_builtin_tool` before dispatch.
  - Rationale: one gate covers both write tools; parser can still extract a path for diagnostics; valid JSON empty writes stay allowed.
  - Alternatives considered: refuse zero-byte overwrite inside `execute_write_file` for all callers; require atomic write + backup.
  - Why rejected: would change intentional empty-file writes; broader than the recovery bug.

## Open Questions

- [x] Should whitespace-only recovered content be rejected? No — only empty/missing `content`.
- [x] Are docs required? No user-facing command/env/security-policy change; recovery of partial content is unchanged.
