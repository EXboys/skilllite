# TASK Card

## Metadata

- Task ID: `TASK-2026-045`
- Title: Roll back Phase 1A WorkspaceService extraction
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Phase 1A bootstrap (TASK-2026-043) created an empty `skilllite-services` crate; Phase 1A real (TASK-2026-044) added `WorkspaceService` and migrated four callsites. Post-implementation review showed:

1. The migration **increased** total LOC in callers (~5 lines per site → ~10–15 lines per site), because the new service returned `Result<…>` over an infallible underlying call, forcing each caller to add `unwrap_or_else` fallback boilerplate.
2. A grep-driven verification of upcoming phases found that the cross-entry duplication those phases were meant to absorb is much smaller than initially estimated:
   - Phase 1B `RuntimeService`: **CLI has zero callers** of `probe_runtime_for_ui` / `provision_runtimes_to_cache` / `get_runtime_dir`. Only Desktop consumes them. The "shared" premise was wrong.
   - Phase 2 `EvolutionService`: 11 shared API references between `commands/evolution.rs` and `bridge/integrations/evolution_ui.rs`, but they are mostly primitive calls (`open_evolution_db`, `EvolutionMode::from_env`) over an already well-shaped `skilllite-evolution` crate, not multi-step flows that need a service-layer wrapping.

Continuing the layered architecture would have been pattern-driven rather than evidence-driven. This TASK rolls back the layer while preserving the genuinely independent Phase 0 boundary work.

## Scope

- In scope:
  - Revert four callsites to call `skilllite_core::skill::discovery` directly (commands/skill/common.rs, commands/init.rs, commands/ide.rs, assistant/bridge/integrations/shared.rs).
  - Remove `skilllite-services` from `crates/skilllite-commands/Cargo.toml` and `crates/skilllite-assistant/src-tauri/Cargo.toml`.
  - Delete `crates/skilllite-services/` (Cargo.toml + src/{lib,error,workspace}.rs).
  - Remove the `skilllite-services` rule from `deny.toml`. Update the deny.toml header comment.
  - Mark TASK-2026-043 and TASK-2026-044 as `superseded` and append explanation in their `REVIEW.md`.
  - Update `todo/multi-entry-service-layer-refactor-plan.md` with rollback note and reasons.
- Out of scope:
  - Reverting Phase 0 work (TASK-2026-042). The Desktop manifest deny coverage, the assistant-as-wrapper allow-list entries for `skilllite-{agent,sandbox,evolution}`, the EN+ZH doc updates, and the new CI step are independent of the services layer and remain in force.
  - Reverting the drive-by `clippy::needless_return` fix in `init.rs::cwd_is_untrusted_for_relative_skills` from TASK-2026-044. That improvement is unrelated to the service layer; keeping it costs nothing.

## Acceptance Criteria

- [x] Four callsites compile and behave identically to their pre-Phase-1A form (verified by file comparison and `cargo test`).
- [x] `crates/skilllite-services/` directory is fully removed.
- [x] `Cargo.toml` of `skilllite-commands` and `skilllite-assistant/src-tauri` no longer reference `skilllite-services`.
- [x] `deny.toml` no longer references `skilllite-services`; its header comment explains the rollback.
- [x] `cargo check --workspace` passes.
- [x] `cargo test --workspace` passes — same suites green as before TASK-2026-044.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings` passes.
- [x] `cargo fmt --check` passes (workspace + Desktop manifest).
- [x] `cargo deny check bans` passes (root + Desktop manifest).
- [x] `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` passes.
- [x] `python3 scripts/validate_tasks.py` passes for this TASK directory.
- [x] TASK-2026-043 and TASK-2026-044 `Status` updated to `superseded`; their `REVIEW.md` includes a "Superseded note" pointing to this TASK.
- [x] `todo/multi-entry-service-layer-refactor-plan.md` carries an explicit rollback record.
- [x] `tasks/board.md` re-read after status change to confirm the entry actually reflects current state.

## Risks

- Risk: A consumer outside the workspace already relied on `skilllite-services` published API.
  - Impact: Build breakage downstream.
  - Mitigation: `skilllite-services` was never published — it only existed in this repo for ~2 batches and was not referenced by Python SDK, MCP, or any external project.
- Risk: The Phase 0 boundary work (deny rules, doc updates) accidentally gets reverted.
  - Impact: Loss of CI enforcement for Desktop manifest layering.
  - Mitigation: This TASK explicitly preserves all Phase 0 deny entries and the Desktop manifest CI invocation; only the `skilllite-services` rule and pre-declared wrappers are removed.
- Risk: Future maintainers re-attempt the same extraction without seeing this audit trail.
  - Impact: Wasted effort.
  - Mitigation: TASK-2026-043 and TASK-2026-044 are kept on disk and on board (status `superseded`) with explicit `REVIEW.md` notes; `todo/multi-entry-service-layer-refactor-plan.md` carries the rollback explanation.

## Validation Plan

- Required tests:
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `cargo fmt --check` (workspace + Desktop manifest)
  - `cargo deny check bans`
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`
  - `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `python3 scripts/validate_tasks.py`
- Commands to run: see "Required tests".
- Manual checks:
  - Re-read `tasks/board.md` after status update.
  - Confirm Phase 0 artifacts (CI step, EN+ZH docs, deny.toml wrapper allow-list for assistant) remain intact.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-services/` (deleted)
  - `crates/skilllite-commands/Cargo.toml` (`skilllite-services` dep removed)
  - `crates/skilllite-commands/src/{skill/common.rs,init.rs,ide.rs}` (callsites reverted)
  - `crates/skilllite-assistant/src-tauri/Cargo.toml` (`skilllite-services` dep removed)
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs` (callsite reverted)
  - `deny.toml` (rule removed; header comment updated)
  - `tasks/TASK-2026-043-services-bootstrap-crate/{TASK.md,REVIEW.md}` (status + note)
  - `tasks/TASK-2026-044-services-phase1a-workspace/{TASK.md,REVIEW.md}` (status + note)
  - `todo/multi-entry-service-layer-refactor-plan.md` (rollback record)
  - `tasks/board.md`
- Explicit non-goals:
  - No changes to runtime behaviour, CLI commands, env vars, Tauri commands, or MCP tools.
  - No reversion of Phase 0 work.

## Links

- Source TODO section: `todo/multi-entry-service-layer-refactor-plan.md` §6.4 ("回滚原则") and the new "事后回滚" block at the top.
- Predecessor TASKs: `TASK-2026-042-services-phase0-decisions` (kept), `TASK-2026-043-services-bootstrap-crate` (superseded), `TASK-2026-044-services-phase1a-workspace` (superseded).
- Related docs: `spec/architecture-boundaries.md`, `spec/verification-integrity.md` (anti-false-positive — this rollback is the response to a self-review under that spec).
