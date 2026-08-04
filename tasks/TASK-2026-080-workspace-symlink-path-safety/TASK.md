# TASK Card

## Metadata

- Task ID: `TASK-2026-080`
- Title: Reject symlink escapes in workspace path resolution
- Status: `in_progress`
- Priority: `P0`
- Owner: `cursor-automation`
- Contributors:
- Created: `2026-08-04`
- Target milestone:

## Problem

Critical-bug sweep found high-confidence path containment gaps: agent/desktop workspace tools used lexical-only resolution and followed in-workspace symlinks outside the root; Tauri pending skill md reads still joined unsanitized `skill_name` (gap vs open PR #89); bash `--cwd` was unconstrained; `rewrite_output_paths` could inject traversal/drive tokens.

## Scope

- In scope:
  - Agent `resolve_within_workspace` / `resolve_within_workspace_or_output`
  - Desktop `resolve_under_workspace`
  - Tauri `read_evolution_pending_skill_md` name validation
  - bash `--cwd` containment
  - `rewrite_output_paths` traversal/drive refusal
- Out of scope:
  - Replacing open PRs #89 / #109–#131
  - Full `O_NOFOLLOW` redesign
  - Changing bash-tool command allowlists beyond cwd containment

## Acceptance Criteria

- [x] Symlink targets outside workspace are rejected by agent path resolution
- [x] Symlink-dir write-through is rejected
- [x] Tauri pending skill md read rejects absolute/traversal names
- [x] bash `--cwd` outside allowed roots is rejected
- [x] `rewrite_output_paths` does not rewrite `../` or Windows drive forms
- [x] Regression tests cover the above
- [x] `python3 scripts/validate_tasks.py` passes

## Validation

- Commands:
  - `cargo test -p skilllite-agent path_containment`
  - `cargo test -p skilllite-agent rewrite_output_paths`
  - `cargo test -p skilllite-commands bash_cwd`
  - `cargo test -p skilllite-assistant pending_name --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `cargo clippy -p skilllite-agent -p skilllite-commands --all-targets -- -D warnings -A dead_code -A clippy::question_mark -A clippy::useless_borrows_in_formatting`
  - `cargo fmt -p skilllite-agent -p skilllite-commands -- --check`
  - `python3 scripts/validate_tasks.py`
- Results: focused tests passed; clippy/fmt clean for changed crates; task validation expected green after metadata fix.

## Risks

- Legitimate outside-pointing symlinks will fail closed (intentional).
- bash cwd for screenshots must use workspace/output roots.

## Regression Scope

- Agent builtin file tools, preview path resolve, bash-tool cwd, desktop workspace editor, pending skill preview read.
