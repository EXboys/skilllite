# CONTEXT

## Technical boundaries

- Shared helper lives in `skilllite-core::path_validation` so metadata, commands, and agent share one rule.
- Metadata parse: reject escaping front-matter values (do not accept them into `SkillMetadata.entry_point`); fall through to directory convention detection.
- `run_skill` / agent override: only accept overrides that pass the same helper and exist as files.
- Final gate in `run_skill`: canonicalize joined entry path and require `starts_with(skill_path)` (parity with `exec_script`).

## Compatibility

- Supported entry points remain skill-relative nested paths.
- Absolute entry points were never safe under level 1 and are rejected.

## Related open work

- PR #127 hardens evolution **write** of generated scripts; this task hardens **runtime execution**.
