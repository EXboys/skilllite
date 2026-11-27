# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-evolution/src/skill_synth/mod.rs` (`confirm_pending_skill`, `reject_pending_skill`)
  - `crates/skilllite-commands/src/evolution.rs` (`cmd_confirm`, `cmd_reject`)
  - `crates/skilllite-commands/src/evolution_desktop.rs` (desktop wrappers)
  - `skilllite-assistant` calls the CLI, so it inherits this check.
- Current behavior: `pending_dir.join(skill_name)` then `remove_dir_all` or `rename`. `Path::join` drops `pending_dir` when `skill_name` is absolute.

## Architecture Fit

- Layer boundaries involved: evolution skill synthesis. The commands crate already calls these functions and does not grow a second copy of the check.
- Interfaces to preserve: `confirm_pending_skill` and `reject_pending_skill` signatures.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: names that are already one directory component behave as before. Docs stay unchanged because the documented command still confirms or rejects one pending skill name. Escaping names were never a supported name shape.

## Design Decisions

- Decision: require exactly one `Component::Normal`, and require that component's text to equal the original name.
  - Rationale: this rejects absolute paths, `.`, `..`, extra components, and a trailing separator. A non-ASCII name is still one normal component.
  - Alternatives considered: canonicalize the joined path and check that it stays inside `_pending`.
  - Why rejected: the dangerous join happens before a safe child exists. Refusing the name avoids following a path that already points outside.

## Open Questions

- [x] Docs sync is not required. Valid pending names keep the same command behavior. No command, flag, env var, or architecture change.
- [x] `read_pending_skill_md` can still join a traversal name, but it only reads `<path>/SKILL.md` and does not delete. Left out of this fix.
