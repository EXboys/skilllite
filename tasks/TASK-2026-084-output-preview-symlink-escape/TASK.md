# TASK Card

## Metadata

- Task ID: `TASK-2026-084`
- Title: Reject symlink escapes in write_output and preview
- Status: `done`
- Priority: `P0`
- Owner: `cursor-automation`
- Contributors:
- Created: `2026-11-28`
- Target milestone:

## Problem

`write_output` and the preview file server only compared a lexically normalized path with `starts_with`. A symlink inside the output or preview directory was treated as in-bounds, then `std::fs::write` / `read_bytes` followed it. Before the fix, `write_output` of `leak` returned `Successfully wrote 5 bytes to .../output/leak` and replaced the symlink target.

## Scope

- In scope:
  - `write_output` overwrite and append
  - Default `<workspace>/output` when that directory is itself a symlink
  - Preview HTTP file resolution, including encoded `..` and symlink files or directories
- Out of scope:
  - Replacing open PR #132 workspace resolver changes
  - `write_file` / `search_replace` lexical resolution (covered by #132)
  - Explicit `SKILLLITE_OUTPUT_DIR` values that the user points outside the workspace

## Acceptance Criteria

- [x] A symlink file inside `output/` cannot be overwritten or appended through `write_output`
- [x] A new file under a symlinked output subdirectory is rejected and is not created outside
- [x] A default `output` directory that is a symlink to another directory is rejected
- [x] A normal Unicode file such as `报告.md` still writes inside `output/`
- [x] Preview resolution rejects an outside symlink file and an outside symlink directory
- [x] Preview resolution still rejects encoded parent traversal and still serves an in-directory Unicode file
- [x] `python3 scripts/validate_tasks.py` passes

## Risks

- Risk: Legitimate symlinks whose targets stay inside the same root are still allowed.
  - Impact: In-tree aliases keep working.
  - Mitigation: The check compares canonical paths with `Path::starts_with`.
- Risk: An explicit `SKILLLITE_OUTPUT_DIR` outside the workspace remains writable.
  - Impact: That override is the existing opt-in output location.
  - Mitigation: Symlinks that leave that configured root are still rejected.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent --lib`
- Commands to run:
  - `cargo fmt -p skilllite-agent -- --check`
  - `cargo clippy -p skilllite-agent --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent --lib`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm the pre-fix `write_output` test returned success and named `output/leak`

## Regression Scope

- Areas likely affected:
  - `write_output`
  - `preview_server` file reads
- Explicit non-goals:
  - Sandbox nftables, session prefix, artifact temp paths, and pending skill names already covered by open PRs

## Links

- Source TODO section:
- Related PRs/issues: open PR #132 covers agent `resolve_within_workspace`, not these two call sites
- Related docs:
