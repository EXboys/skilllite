# PRD

## Background

After Phase 0 decisions (TASK-2026-042) and Phase 1A crate bootstrap (TASK-2026-043), this TASK lands the first real shared service in `skilllite-services`. The chosen seed is the workspace skills-directory resolution flow because it is the smallest concrete duplication (4 callsites, ~5 lines each) and has no I/O or async concerns — making it a low-risk vehicle to validate the service-layer pattern and the CLI/Desktop adapter conventions before larger Phase 1B/2 migrations begin.

## Objective

Land a single `WorkspaceService::resolve_skills_dir` consumed by both CLI (`skilllite-commands`) and Desktop (`skilllite-assistant/src-tauri`) entries, eliminate the four duplicated wrappers, and demonstrate the service-layer migration loop end-to-end (service implementation → CLI adapter → Desktop adapter → quality gates → TASK closure) without changing any observable behaviour.

## Functional Requirements

- FR-1: Implement `WorkspaceService` in `crates/skilllite-services/src/workspace.rs` with sync `resolve_skills_dir` and `resolve_skills_dir_for_workspace` methods.
- FR-2: Define `ResolveSkillsDirRequest` / `ResolveSkillsDirResponse` with `serde::{Serialize, Deserialize}` derives and stable field shape, including a structured `conflict_warning: Option<String>` and `conflicting_skill_names: Vec<String>`.
- FR-3: Replace three CLI wrappers (`commands/skill/common.rs`, `commands/init.rs`, `commands/ide.rs`) with calls into `WorkspaceService`, preserving the existing `eprintln!` of `conflict_warning`.
- FR-4: Replace the Desktop bridge wrapper (`bridge/integrations/shared.rs::resolve_workspace_skills_root`) with calls into `WorkspaceService`, preserving the existing silent-drop behaviour.
- FR-5: Cover the service with at least 6 unit tests (invalid input, blank input, legacy fallback, primary path, conflict warning, absolute path).

## Non-Functional Requirements

- Security: No security-relevant change.
- Performance: No measurable runtime impact (operations are local filesystem reads identical to before).
- Compatibility: All four CLI subcommands and all Desktop Tauri commands must continue to behave identically. CLI stderr conflict-warning text must remain bit-identical with the previous output.

## Constraints

- Technical:
  - Must comply with `spec/architecture-boundaries.md` (services can only be consumed by entry-layer crates; deny.toml rule from Phase 0 D2 enforces this).
  - Must comply with `spec/rust-conventions.md` (no `unsafe`, per-crate `thiserror`, no raw `anyhow` in service crate).
  - Must comply with `spec/testing-policy.md` (unit tests cover both happy path and at least one rejection path).
  - Must comply with `spec/task-artifact-language.md` (English task artifacts).
  - `cargo deny check bans` must continue to pass against both root and Desktop manifest.
- Timeline: This TASK is a prerequisite for Phase 1B (Runtime) only in that it sets the convention; Phase 1B can start in parallel if needed.

## Success Metrics

- Metric: Number of duplicated CLI/Desktop wrappers around `skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback`.
  - Baseline: 4 (3 CLI + 1 Desktop).
  - Target: 0 unique wrapper definitions; all four sites call `WorkspaceService`.
- Metric: New unit test count in `skilllite-services`.
  - Baseline: 0.
  - Target: ≥ 6.
- Metric: `cargo deny check bans` `unused-wrapper` warnings for the `skilllite-services` rule.
  - Baseline: 3 unmatched wrappers per graph (Phase 1A bootstrap state).
  - Target: ≤ 1 unmatched wrapper per graph (real consumers wired in).

## Rollout

- Rollout plan: Single PR adds the service implementation and migrates all four callsites; no version bump; no behaviour change.
- Rollback plan: Revert the PR; the `BOOTSTRAP_PHASE` constant goes back, the four callsites again call core directly, and `cargo deny`'s `unused-wrapper` count returns to the Phase 1A bootstrap baseline.
