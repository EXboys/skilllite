# TASK Card

## Metadata

- Task ID: `TASK-2026-044`
- Title: Extract WorkspaceService into skilllite-services
- Status: `done`

- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Phase 0 (TASK-2026-042) decided to introduce `skilllite-services` as the entry-neutral home for shared application services; Phase 1A bootstrap (TASK-2026-043) created the empty crate. Today, three CLI sites in `skilllite-commands` and one Desktop bridge site each independently wrap `skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback` and handle the conflict warning differently (CLI prints to stderr, Desktop silently drops). This duplication blocks any future change to the resolution behaviour from being applied uniformly across entries.

## Scope

- In scope:
  - Implement `WorkspaceService::resolve_skills_dir` (sync, per Phase 0 D3 documented exception) in `crates/skilllite-services/`.
  - Define `ResolveSkillsDirRequest` / `ResolveSkillsDirResponse` as serde-serializable plain data (Phase 0 D5).
  - Use per-crate `thiserror` (Phase 0 D3) error type.
  - Cover service with unit tests (happy path + rejection + legacy fallback + conflict warning).
  - Migrate three CLI callsites (`skill::common::resolve_skills_dir`, `init::resolve_path_with_legacy_fallback`, `ide::resolve_skills_dir_with_legacy_fallback`) to consume the service.
  - Migrate the Desktop bridge callsite (`integrations::shared::resolve_workspace_skills_root`) to consume the service.
  - Remove the `BOOTSTRAP_PHASE` placeholder constant and rewrite the crate-level rustdoc to reflect Phase 1A.
- Out of scope:
  - Changing observable CLI / Desktop behaviour. Conflict warning still printed to stderr by CLI and still silently dropped by Desktop (a follow-up TASK can wire it into the assistant UI).
  - Migrating `discover_skill_instances_in_workspace` filtering (`integrations::shared::discover_scripted_skill_instances`) — the duplication there is too thin to justify a service yet.
  - Touching `find_project_root` (CLI uses `current_dir`, Desktop walks up to a `Documents/SkillLite` fallback; intentionally divergent).
  - Phase 1B `RuntimeService` and any work past Phase 1A.

## Acceptance Criteria

- [x] `crates/skilllite-services/src/error.rs` defines `Error` (thiserror) + `Result` alias.
- [x] `crates/skilllite-services/src/workspace.rs` defines `WorkspaceService`, `ResolveSkillsDirRequest`, `ResolveSkillsDirResponse` with serde derives, and 6 unit tests covering invalid input, legacy fallback, primary path, conflict warning, and absolute path.
- [x] `crates/skilllite-services/src/lib.rs` exports the new types and removes `BOOTSTRAP_PHASE`.
- [x] `crates/skilllite-commands/Cargo.toml` adds the `skilllite-services` path dep.
- [x] `crates/skilllite-commands/src/skill/common.rs::resolve_skills_dir` calls `WorkspaceService` (with stderr-print fallback intact).
- [x] `crates/skilllite-commands/src/init.rs::resolve_path_with_legacy_fallback` calls `WorkspaceService` (with stderr-print fallback intact).
- [x] `crates/skilllite-commands/src/ide.rs::resolve_skills_dir_with_legacy_fallback` calls `WorkspaceService` (returning the new `ResolveSkillsDirResponse`).
- [x] `crates/skilllite-assistant/src-tauri/Cargo.toml` adds the `skilllite-services` path dep.
- [x] `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs::resolve_workspace_skills_root` calls `WorkspaceService` (preserving silent-drop conflict-warning behaviour).
- [x] `cargo test --workspace` passes — workspace summary lists multiple test suites with no failures.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes (cleaned the pre-existing `init.rs:28` `needless_return` warning while in the file).
- [x] `cargo fmt --check` passes (workspace + Desktop manifest).
- [x] `cargo deny check bans` passes (root + Desktop manifest).
- [x] `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` passes.
- [x] `python3 scripts/validate_tasks.py` passes for this TASK directory.
- [x] `tasks/board.md` re-read after status change to confirm the entry actually reflects current state.

## Risks

- Risk: CLI stderr-print of conflict warning regresses (a user relying on that exact output).
  - Impact: Tooling parsing CLI stderr could miss the warning.
  - Mitigation: Behaviour preserved verbatim — CLI `eprintln!` of the warning is now driven by `response.conflict_warning` whose value is identical to the prior `resolution.conflict_warning()`.
- Risk: Desktop bridge silent-drop now goes through the service abstraction; introducing a logging side-effect would change observable behaviour.
  - Impact: Unintended log noise in Desktop tray app.
  - Mitigation: Implementation deliberately omits any `tracing` / `eprintln!` in `resolve_workspace_skills_root`'s service path (the Desktop crate has no `tracing` dep on purpose).
- Risk: Pre-existing `cargo deny` `unused-wrapper` warnings remain (notably one wrapper in the `skilllite-services` rule per graph).
  - Impact: Visual noise in CI output.
  - Mitigation: Documented as expected; warnings progressively reduce as future entries (MCP) consume the service.

## Validation Plan

- Required tests:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `cargo fmt --check --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `cargo deny check bans`
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`
  - `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `python3 scripts/validate_tasks.py`
- Commands to run: see "Required tests".
- Manual checks:
  - Re-read `tasks/board.md` after status update.
  - Confirm the new service rustdoc references Phase 0 D3 / D5 design rationale.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-services/` (new module + new public types)
  - `crates/skilllite-commands/src/{skill/common.rs,init.rs,ide.rs}` (callsite migration only — no behaviour change)
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs` (callsite migration only — no behaviour change)
- Explicit non-goals:
  - No changes to runtime behaviour, CLI commands, env vars, Tauri commands, or MCP tools.
  - No changes to `find_project_root` or skills discovery.

## Links

- Source TODO section: `todo/multi-entry-service-layer-refactor-plan.md` Phase 1A.
- Predecessor TASKs: `tasks/TASK-2026-042-services-phase0-decisions/`, `tasks/TASK-2026-043-services-bootstrap-crate/`.
- Next TASK: `services-phase1b-runtime` — async `RuntimeService` (probe / provision / diagnostics).
- Related docs: `spec/architecture-boundaries.md`, `spec/rust-conventions.md`, `spec/testing-policy.md`, `spec/structured-signal-first.md` (`conflict_warning` field is the structured signal; the formatted string is convenience), `crates/skilllite-services/src/workspace.rs` rustdoc.
