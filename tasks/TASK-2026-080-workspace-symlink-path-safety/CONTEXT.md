# CONTEXT — Workspace symlink / path containment

## Technical boundaries

- Agent builtins: `resolve_within_workspace` / `resolve_within_workspace_or_output` in `helpers.rs`.
- Desktop editor: `resolve_under_workspace` in assistant `workspace.rs` (same symlink class).
- Pending read gap: Tauri `pending.rs` does **not** call `evolution_desktop::read_pending_skill_md`; PR #89 alone does not close this path.
- bash cwd: `execute_bash_with_env` previously set `current_dir` for any existing directory.

## Compatibility

- Fail-closed for outside symlink targets.
- Nested relative paths under a real in-tree directory remain allowed.
- bash cwd under skill/workspace/output/process cwd remains allowed.

## Related open PRs (do not rediscover)

#89, #109–#120, #121, #123–#131
