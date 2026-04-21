# TASK Card

## Metadata

- Task ID: `TASK-2026-043`
- Title: Bootstrap empty skilllite-services crate
- Status: `cancelled`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Phase 0 (TASK-2026-042) decided to introduce a new `skilllite-services` crate as the entry-neutral home for shared application services (D2). Before any real service migration can begin (Phase 1A WorkspaceService extraction), the crate itself must exist as a workspace member, compile cleanly, and activate the pre-declared `cargo deny` boundary rule. Without this prerequisite, Phase 1A would mix "create a new crate" risk with "migrate live business logic" risk in the same PR.

## Scope

- In scope:
  - Create `crates/skilllite-services/Cargo.toml` and `crates/skilllite-services/src/lib.rs` with no business logic.
  - Confirm the workspace `crates/*` glob picks up the new crate without modifying root `Cargo.toml`.
  - Verify `cargo check`, `cargo fmt --check`, `cargo clippy -- -D warnings` all pass on the new crate.
  - Verify both `cargo deny check bans` invocations (root + Desktop manifest) still pass.
  - Document the crate's purpose, status, and boundary policy in `lib.rs` rustdoc.
- Out of scope:
  - Any service implementation (workspace probe, runtime, evolution, session, chat). All deferred to follow-up TASKs starting with `services-phase1a-workspace`.
  - Any code migration from `skilllite-commands` or `skilllite-bridge`.
  - Any change to existing crate behavior, CLI commands, env vars, or Tauri commands.
  - Any new runtime dependencies (no `tokio`, `serde`, `thiserror`, etc. in this crate yet).

## Acceptance Criteria

- [x] `crates/skilllite-services/Cargo.toml` exists, uses workspace package fields, has empty `[dependencies]`, and includes a header comment linking back to `TASK-2026-042` decisions and this TASK.
- [x] `crates/skilllite-services/src/lib.rs` exists with `forbid(unsafe_code)`, `deny(rust_2018_idioms)`, `warn(missing_docs)`, a documented `BOOTSTRAP_PHASE` const, and a header docstring describing scope, status, and boundary rules per Phase 0 D1..D5.
- [x] `cargo check -p skilllite-services` succeeds with no warnings.
- [x] `cargo check --workspace` succeeds (no regression on existing crates).
- [x] `cargo fmt --check -p skilllite-services` produces no diff.
- [x] `cargo clippy -p skilllite-services --all-targets -- -D warnings` succeeds.
- [x] `cargo deny check bans` (root) succeeds (`bans ok`); only `unused-wrapper` warnings remain and are documented as expected.
- [x] `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans` succeeds (`bans ok`).
- [x] `python3 scripts/validate_tasks.py` passes for this TASK directory.
- [x] `tasks/board.md` re-read after status change to confirm the entry actually reflects current state.

## Risks

- Risk: `crates/*` workspace glob picks up the new crate but breaks builds on a CI runner with a different filesystem case-sensitivity setup.
  - Impact: PR fails CI but local passes.
  - Mitigation: New crate name and path are lowercase ASCII; verified by `cargo check --workspace` locally.
- Risk: Empty `[dependencies]` triggers a future warning if Cargo introduces a stricter check.
  - Impact: Warning on future toolchain bumps.
  - Mitigation: Acceptable for a deliberate placeholder crate; the next TASK adds real deps.
- Risk: Pre-declared `deny.toml` rule for `skilllite-services` keeps emitting `unused-wrapper` warnings until the first real consumer wires in.
  - Impact: Visual noise in CI output.
  - Mitigation: Documented as expected in `REVIEW.md` and in `deny.toml` comments.

## Validation Plan

- Required tests:
  - `cargo check -p skilllite-services`
  - `cargo check --workspace`
  - `cargo fmt --check -p skilllite-services`
  - `cargo clippy -p skilllite-services --all-targets -- -D warnings`
  - `cargo deny check bans`
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`
  - `python3 scripts/validate_tasks.py`
- Commands to run: see "Required tests".
- Manual checks:
  - Re-read `tasks/board.md` after status update.
  - Confirm `crates/skilllite-services/src/lib.rs` rustdoc covers Phase 0 D1..D5.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-services/` (new crate, additive only)
  - Workspace build graph (no behavior change)
- Explicit non-goals:
  - No changes to runtime behavior, CLI commands, env vars, Tauri commands, or MCP tools.
  - No migration of existing logic.

## Links

- Source TODO section: `todo/multi-entry-service-layer-refactor-plan.md` Phase 0 §4.2 (D2) and Phase 1A introduction.
- Predecessor TASK: `tasks/TASK-2026-042-services-phase0-decisions/`
- Next TASK after this one: `services-phase1a-workspace` (real WorkspaceService extraction).
- Related docs: `spec/architecture-boundaries.md`, `spec/rust-conventions.md`, `spec/testing-policy.md`, `crates/skilllite-services/Cargo.toml`, `crates/skilllite-services/src/lib.rs`.
