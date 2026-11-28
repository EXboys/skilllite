# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/builtin/output.rs`
  - `crates/skilllite-agent/src/extensions/builtin/preview.rs`
  - `crates/skilllite-agent/src/extensions/builtin/helpers.rs`
- Current behavior: `normalize_path` removes lexical `..`. `Path::starts_with` then accepts any symlink whose link path sits inside the root. `skilllite_fs::write_file` and `read_bytes` follow that symlink.

## Architecture Fit

- Layer boundaries involved: agent builtin tools only. No new crate dependency.
- Interfaces to preserve: `write_output` arguments, preview bind address `127.0.0.1`, and explicit `SKILLLITE_OUTPUT_DIR`.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: Outside-pointing symlinks now fail closed. In-root files, including `报告.md`, still succeed. Docs and env defaults are unchanged because the published contract already rejected paths that escape the output directory; this closes the symlink hole in that check.

## Design Decisions

- Decision: Canonicalize the root and the nearest existing ancestor, then require `starts_with`.
  - Rationale: A not-yet-created file under a symlinked directory has no canonical path of its own. The ancestor is the path `create_dir_all` / `write` would follow.
  - Alternatives considered: Reuse the unmerged `resolve_under_root` helper from PR #132.
  - Why rejected: That PR does not patch `execute_write_output` or `resolve_preview_file`. Copying it would duplicate a large unmerged change.
- Decision: Check the default output directory against the workspace only when `SKILLLITE_OUTPUT_DIR` is unset.
  - Rationale: The env override is the existing way to choose an output directory outside the workspace.
  - Alternatives considered: Always require the output root to stay inside the workspace.
  - Why rejected: That would break the documented override.

## Open Questions

- [x] None for this fix.
