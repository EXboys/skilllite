# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Fail closed truncated write_file recovery
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-08-18`
- Target milestone:

## Problem

Truncated / invalid `write_file` and `write_output` JSON recovery can destroy existing files:

1. Chunked writes document `append: true` after the first overwrite. When the stream is cut inside `content`, recovery never sees `append: true` and overwrites the file with the partial chunk.
2. When `content` precedes `path` and the content contains an unescaped `"path"` field (common when writing JSON/config), recovery writes to the inner path instead of the intended file.

## Scope

- In scope:
  - Recover path/file_path only from the JSON prefix before the first `"content"` key.
  - Refuse recovered overwrites of existing non-empty files unless `append: true` was recovered.
  - Regression tests for both trigger scenarios plus preserved happy paths.
- Out of scope:
  - Empty recovered content wipe (open PR #144).
  - Atomic writes, symlink containment, dotenv variants (open PRs).
  - Broad recovery parser rewrite.

## Acceptance Criteria

- [x] Existing file + truncated write without `append: true` returns an error and leaves the file unchanged.
- [x] Content-first JSON with an inner `"path"` does not write to that inner path.
- [x] Recovered write to a new file still succeeds with the truncation warning.
- [x] Recovered write with explicit `append: true` before `content` still appends.
- [x] Valid JSON overwrite / append behavior is unchanged.

## Risks

- Risk: Fail-closed recovery rejects a truncated overwrite the agent intended.
  - Impact: Agent must retry with complete JSON instead of keeping a partial rewrite.
  - Mitigation: Preserve recovery for new files and for explicit `append: true`. Valid JSON is unchanged.
- Risk: Overlap with PR #144 empty-content gate.
  - Impact: Merge conflict in `execute_builtin_tool`.
  - Mitigation: Keep this change local to path prefix matching and existing-file clobber check; do not reimplement the empty-content gate.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent` including new recovered-write tests; workspace `cargo test`; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `python3 scripts/validate_tasks.py`.
- Commands to run:
  - `cargo test -p skilllite-agent recovered_`
  - `cargo test -p skilllite-agent`
  - `cargo test`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Trace `execute_builtin_tool` → `parse_truncated_json_for_file_tools` → `execute_write_file` / `execute_write_output`.
  - Re-read edited files after the change.

## Regression Scope

- Areas likely affected: `write_file` / `write_output` truncated-JSON recovery.
- Explicit non-goals: Empty recovered content (#144), sandbox, evolution restore, desktop chat roots.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-08-18.
- Related PRs/issues: Open PR #144 (empty recovered write); this change covers #144 explicit non-goal (inner `"path"` when content precedes path) plus lost `append: true`.
- Related docs: `spec/verification-integrity.md`, `spec/task-artifact-language.md`, `spec/testing-policy.md`, `spec/rust-conventions.md`.
