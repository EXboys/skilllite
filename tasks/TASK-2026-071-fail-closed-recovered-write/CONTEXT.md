# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/builtin/mod.rs` (`execute_builtin_tool`)
  - `crates/skilllite-agent/src/extensions/builtin/helpers.rs` (`parse_truncated_json_for_file_tools`)
  - `crates/skilllite-agent/src/extensions/builtin/file_ops/mod.rs` (`execute_write_file`)
  - `crates/skilllite-agent/src/extensions/builtin/output.rs` (`execute_write_output`)
- Current behavior:
  - Invalid JSON for `write_file` / `write_output` is recovered via regex.
  - The first `"path"` / `"file_path"` match anywhere in the blob is used.
  - `append` is true only when the literal `"append":true` appears; otherwise overwrite.

## Architecture Fit

- Layer boundaries involved: agent builtin tools only. No crate dependency direction change.
- Interfaces to preserve: `execute_builtin_tool` signature and valid-JSON write semantics.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes:
  - Truncated overwrite of an **existing** file without recovered `append: true` now errors instead of clobbering. That is intentional fail-closed behavior.
  - Truncated create of a **new** file is unchanged.

## Design Decisions

- Decision: Scan `path` / `file_path` only in the prefix before the first `"content"` key.
  - Rationale: Top-level path keys are emitted before content in the documented path-first shape. Inner `"path"` values live inside content.
  - Alternatives considered: Take the last `"path"` match in the whole blob.
  - Why rejected: Content can contain later `"path"` fields; last-match still writes to the wrong file.
- Decision: Refuse recovered overwrite of existing non-empty files unless `append: true` was recovered.
  - Rationale: Seed docs tell the model to append after the first chunk. Truncation almost always happens inside `content`, so `append` is lost and the first chunk is destroyed.
  - Alternatives considered: Default recovered writes to append-if-exists.
  - Why rejected: Silent append on an intended overwrite mixes old and new bytes without a clear error. Fail-closed asks the agent to retry.

## Open Questions

- [x] Should empty recovered content be gated here? No — that is PR #144.
- [x] Docs sync? No — recovery is not a user-facing command/env/API change.
