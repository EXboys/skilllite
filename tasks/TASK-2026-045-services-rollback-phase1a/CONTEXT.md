# Technical Context

## Current State (pre-rollback)

- `crates/skilllite-services/` exists with `Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/workspace.rs` (~250 lines incl. 6 unit tests).
- `crates/skilllite-commands/Cargo.toml` and `crates/skilllite-assistant/src-tauri/Cargo.toml` each list `skilllite-services` as a path dep.
- Four callsites consume `WorkspaceService::resolve_skills_dir_for_workspace`:
  - `crates/skilllite-commands/src/skill/common.rs::resolve_skills_dir`
  - `crates/skilllite-commands/src/init.rs::resolve_path_with_legacy_fallback`
  - `crates/skilllite-commands/src/ide.rs::resolve_skills_dir_with_legacy_fallback` (private helper)
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs::resolve_workspace_skills_root`
- `deny.toml` contains a pre-declared `skilllite-services` rule plus `skilllite-services` listed as an allowed wrapper for many other denied crates.

## Architecture Fit

- Layer boundaries involved: Removing `skilllite-services` returns the dependency graph to `entry → commands/agent → executor → sandbox → core` (with `assistant` as a parallel entry per Phase 0 D1).
- Interfaces to preserve:
  - All CLI subcommand behaviour (stdout/stderr text, exit codes).
  - All Tauri commands exposed by `skilllite_bridge`.
  - All MCP tool schemas.
  - All Python SDK subprocess/IPC behaviour.

## Dependency and Compatibility

- Removed dependencies: `skilllite-services` from `skilllite-commands` and `skilllite-assistant/src-tauri`.
- `Cargo.lock` will lose the `skilllite-services` entry; this is a no-op for downstream tooling.
- `cargo deny check bans` continues to pass against both manifests with the same wrapper allow-lists Phase 0 already established (assistant remains an allowed wrapper for agent/sandbox/evolution; only the `skilllite-services` rule is removed).

## Design Decisions

- Decision — Roll back service layer rather than push through to Phase 2.
  - Rationale: A `WorkspaceService` extraction that net-increases caller LOC is not "establishing a pattern", it is anchoring a poor pattern. Future maintainers would copy the `unwrap_or_else` fallback shape into new services and propagate the same problem.
  - Alternatives considered:
    - A. Keep everything, do not continue to Phase 2 (R1).
    - B. Roll back Phase 1A only, keep Phase 0 (this TASK, R2).
    - C. Roll back everything including Phase 0 (R3).
    - D. Push through to Phase 2 EvolutionService to "justify" the layer (R4).
  - Why rejected:
    - A: Leaves a thin `skilllite-services` crate as future-maintenance attractor; eventual cleanup is harder.
    - C: Loses Phase 0's independently valid CI/doc improvements for no reason.
    - D: Phase 2's actual overlap (per grep) is small and primitive-level; does not change the cost/benefit verdict; doubles down on a weak premise.

- Decision — Keep TASK-2026-043 and TASK-2026-044 on disk with `superseded` status.
  - Rationale: Audit trail for future contributors; deleting them would erase the reasoning sequence.
  - Alternatives considered: Delete the superseded TASK folders.
  - Why rejected: Loses institutional memory of why we did NOT proceed.

- Decision — Keep the drive-by `init.rs::cwd_is_untrusted_for_relative_skills` `needless_return` → expression fix from TASK-2026-044.
  - Rationale: Independent improvement; reverting it would re-introduce a pre-existing clippy warning that has nothing to do with the services layer.
  - Alternatives considered: Revert it for "clean rollback".
  - Why rejected: Cosmetic loss with no benefit.

- Decision — Keep all Phase 0 work (TASK-2026-042) intact.
  - Rationale: Phase 0 is independently valid and grep-verified to be fact-aligned (Desktop genuinely depends on agent/sandbox/evolution; CI deny coverage genuinely catches future violations).
  - Alternatives considered: Roll back Phase 0 too (R3 above).
  - Why rejected: See R3 in the first decision above.

## Open Questions

- [ ] If a future MCP entry crate or a complex multi-step flow truly emerges that benefits from a shared service layer, this rollback does not preclude re-introducing one. At that time the scope and shape should be re-derived from real evidence rather than from the original Phase 1+ plan.
