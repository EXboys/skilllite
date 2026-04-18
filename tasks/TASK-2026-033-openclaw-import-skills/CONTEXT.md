# Context

## Technical

- Implementation: `crates/skilllite-commands/src/skill/import_openclaw.rs`; CLI in `skilllite/src/cli.rs`; dispatch `dispatch/skill.rs`.
- `skilllite_core::skill::metadata` already merges `metadata.openclaw.requires` into compatibility.
- `copy_skill`, `install_skill_deps`, `scan_candidate_skills` re-exported from `skill::add` with `pub(in crate::skill)` for sibling module use.

## Compatibility

- Alias: `import-openclaw` (visible in clap).
- Works with `skilllite-sandbox` binary build (no agent feature required; use `--scan-offline` if LLM admission unavailable).
