# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/skill/add/mod.rs`
  - `crates/skilllite-commands/src/skill/add/discovery.rs`
  - `crates/skilllite-commands/src/skill/remove.rs`
  - `crates/skilllite-commands/src/skill/import_openclaw.rs`
  - `crates/skilllite-evolution/src/skill_synth/mod.rs`
  - `crates/skilllite-evolution/src/skill_synth/generate.rs`
  - `skilllite/src/mcp/handlers.rs`
- Current behavior: `skills_path.join(skill_name)` replaces the base when `skill_name` is absolute and walks out of the root when it contains `..`. `copy_skill` then calls `remove_dir_all` on that destination.

## Architecture Fit

- Layer boundaries involved: `skilllite-core` owns `validate_skill_dir_name`. Commands, evolution, and the MCP entry call it. Dependency direction stays entry -> commands -> evolution -> core.
- Interfaces to preserve: `cmd_add`, `cmd_remove`, and `copy_skill` still install a single-component name under the skills root. `copy_skill` now takes `(skills_root, src, skill_name)`.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Names such as `my-skill`, `nano-banana-pro`, `笔记`, and `计算器` remain valid. `..`, `.`, `../../.config`, and `/tmp/x` are refused.

## Design Decisions

- Decision: Validate the name before `Path::join`, and require the joined path to be exactly one component under the skills root.
  - Rationale: `Path::starts_with` does not undo `..`, so a prefix check on the unnormalized path still accepts an escape.
  - Alternatives considered: Canonicalize after join and compare with the skills root.
  - Why rejected: The destination may not exist yet, and `remove_dir_all` would already have run if the check happened after the delete.

## Open Questions

- [x] None for this fix.
