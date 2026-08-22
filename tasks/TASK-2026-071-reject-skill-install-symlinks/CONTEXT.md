# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/skill/add/discovery.rs` (`copy_skill`, `copy_dir_filtered`)
  - Callers: `skill/add/mod.rs` (`cmd_add`, `update_skill_from_source`), `skill/import_openclaw.rs`
- Current behavior: `Path::is_dir()` follows directory symlinks and recursively copies the target tree; `fs::copy` follows file symlinks and writes target bytes as a regular file in `.skills/`.

## Architecture Fit

- Layer boundaries involved: commands / skill management only (Layer 1–2)
- Interfaces to preserve: `copy_skill(src, dest) -> Result<()>`; public CLI flags unchanged

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: install now errors if a (non-excluded) symlink is present in the skill tree

## Design Decisions

- Decision: fail closed on copied symlinks; skip excluded-dir names even when they are symlinks
  - Rationale: excluded dirs (`.venv`, `node_modules`) are commonly symlinks and are not copied today
  - Alternatives considered: copy as symlink without following; skip-and-warn
  - Why rejected: recreating host-absolute symlinks in `.skills/` still exposes the target at runtime; skip-and-warn can hide a malicious extra file
- Decision: pre-scan before `remove_dir_all(dest)`
  - Rationale: `copy_skill` currently deletes dest first; a mid-copy reject would wipe an existing install
  - Alternatives considered: copy-to-temp then rename
  - Why rejected: larger behavior change; pre-scan is sufficient for this bug

## Open Questions

- [x] Should ZIP symlink entries be handled here? No — zip extract already writes them as regular files containing the path string, not followed content.
- [x] Overlap with PR #132? No — that PR is agent workspace containment, not install copy.
