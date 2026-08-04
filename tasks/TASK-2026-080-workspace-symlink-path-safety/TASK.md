# TASK-2026-080: Reject symlink escapes in workspace path resolution

## Goal

Close critical workspace/path containment gaps found in the 2026-08-04 critical-bug sweep:
1. Agent builtin `read_file`/`write_file` lexical containment followed symlinks outside the workspace.
2. Desktop Tauri `read_evolution_pending_skill_md` joined unsanitized `skill_name` (gap left by open PR #89).
3. stdio/CLI bash `--cwd` accepted arbitrary host directories.
4. Agent `rewrite_output_paths` could inject traversal/drive tokens into absolute paths.

## Scope

- `crates/skilllite-agent/src/extensions/builtin/helpers.rs`
- `crates/skilllite-agent/src/skills/executor.rs` (`rewrite_output_paths`)
- `crates/skilllite-commands/src/execute.rs` (bash cwd containment)
- `crates/skilllite-assistant` pending skill md reader + desktop workspace resolve

## Non-goals

- Merging or replacing open PRs #89, #109–#131
- Full `O_NOFOLLOW` openat redesign
- Changing bash-tool allowlist semantics beyond cwd containment

## Acceptance Criteria

- [x] Symlink targets outside workspace are rejected by agent path resolution
- [x] Symlink-dir write-through (`out/pwn.txt` where `out` -> outside) is rejected
- [x] Tauri pending skill md read rejects absolute/traversal/`..` names
- [x] bash `--cwd` outside skill/workspace/output/process-cwd roots is rejected
- [x] `rewrite_output_paths` does not rewrite `../` or Windows drive forms
- [x] Regression tests cover the above
- [x] `python3 scripts/validate_tasks.py` passes

## Validation

- `cargo test -p skilllite-agent path_containment`
- `cargo test -p skilllite-agent rewrite_output_paths`
- `cargo test -p skilllite-commands bash_cwd`
- `cargo test -p skilllite-assistant pending_name --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
- `cargo clippy -p skilllite-agent -p skilllite-commands --all-targets -- -D warnings -A dead_code -A clippy::question_mark -A clippy::useless_borrows_in_formatting`
- `cargo fmt -p skilllite-agent -p skilllite-commands -- --check`

## Risks

- Legitimate symlinks that point outside the workspace will now fail (intentional fail-closed).
- bash cwd that pointed at arbitrary host dirs for screenshots must use workspace/output roots.

## Regression Scope

- Agent builtin file tools, preview path resolve, bash-tool execution cwd, desktop workspace editor, pending skill preview read.
