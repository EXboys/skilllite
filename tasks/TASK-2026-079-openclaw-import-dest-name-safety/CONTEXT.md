# CONTEXT

## Technical boundaries

- Shared validator lives in `skilllite-core::path_validation` so commands/MCP can reuse one rule (aligned with PR #125).
- `import_openclaw.rs` validates logical names before conflict resolution and re-validates rename destinations before planning.
- Install path uses `skill_dir_under_root` instead of raw `skills_path.join(dest_name)`.

## Compatibility

- Valid single-segment names (including non-ASCII) continue to import unchanged.
- Previously accepted escape names are now skipped.

## Constraints

- Do not broaden into add/MCP/CLI in this PR (avoid overlapping #125 call-site churn).
- Keep helper signatures identical to #125 to reduce merge friction.
