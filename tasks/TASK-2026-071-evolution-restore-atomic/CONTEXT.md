# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-evolution/src/snapshots.rs` (`restore_extended_snapshot`, `copy_dir_recursive`)
  - `crates/skilllite-evolution/src/rollback.rs` (`execute_evolution_rollback` → restore)
  - `crates/skilllite-evolution/src/lib.rs` (existing restore test)
- Current behavior:
  - If `memory/evolution` exists in the snapshot, delete live `chat_root/memory/evolution` then copy.
  - If `skills/_evolved` exists in the snapshot, delete live `skills_root/_evolved` then copy.

## Architecture Fit

- Layer boundaries involved: evolution crate only; commands/agent keep calling `restore_extended_snapshot`.
- Interfaces to preserve: `restore_extended_snapshot(chat_root, skills_root, txn_id)` signature and snapshot directory layout.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: successful restore semantics unchanged; failure semantics become non-destructive for live trees.

## Design Decisions

- Decision: copy snapshot tree to a sibling `.<name>.restore-tmp-<pid>` directory, rename live dest to `.<name>.restore-old-<pid>`, rename temp into dest, then remove backup.
  - Rationale: same-parent `rename` is atomic on POSIX for directories; live data is not deleted until a complete copy exists.
  - Alternatives considered: copy live to backup first then delete dest then copy snapshot into dest.
  - Why rejected: still has a window where dest is missing; sibling rename-swap is shorter and easier to reverse.

## Open Questions

- [x] Should prompt files use the same helper? No — they are individual files copied in place, not a tree replace.
- [x] Docs/env/CLI changes? No.
