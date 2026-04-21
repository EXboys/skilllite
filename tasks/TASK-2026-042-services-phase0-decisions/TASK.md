# TASK Card

## Metadata

- Task ID: `TASK-2026-042`
- Title: Multi-entry service layer Phase 0 decisions
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

The repository now ships four product entries (CLI, Desktop, MCP, Python SDK), but Desktop already directly consumes multiple core crates while documentation and CI rules still describe it as a thin shell. Shared business flows are duplicated between `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/**` and `crates/skilllite-commands/src/**`. Without explicit Phase 0 decisions, any Phase 1 service-layer extraction would be repeatedly reworked.

## Scope

- In scope:
  - Lock the five Phase 0 decisions (D1..D5) referenced in `todo/multi-entry-service-layer-refactor-plan.md`.
  - Update entry/architecture documents to reflect that Desktop is a first-class entry.
  - Define the wrapper allow-list and CI command for the new `skilllite-services` crate boundary (no source code yet).
- Out of scope:
  - Creating the actual `skilllite-services` crate code (deferred to a follow-up TASK in Phase 1A bootstrap).
  - Migrating any business logic from `skilllite-bridge` or `skilllite-commands` (Phase 1A/1B/2 scope).
  - Touching `agent_loop`, chat orchestration, or evolution write paths.

## Acceptance Criteria

- [x] D1 recorded: Desktop is a first-class entry; `docs/en/ENTRYPOINTS-AND-DOMAINS.md` and `docs/zh/ENTRYPOINTS-AND-DOMAINS.md` no longer describe Desktop as "shell over installed binary".
- [x] D2 recorded: a new crate `skilllite-services` is the chosen physical home for shared application services; alternatives are explicitly rejected in `CONTEXT.md`.
- [x] D3 recorded: service interfaces default to `async` (tokio); error model is per-crate `thiserror`; conversion strategy for CLI / Desktop adapters is written down.
- [x] D4 recorded: `cargo deny check bans` is extended to `crates/skilllite-assistant/src-tauri/Cargo.toml`; the exact CI command is documented.
- [x] D5 recorded: service interfaces use serde-serializable plain data, no platform/UI specific types; Python SDK keeps subprocess/IPC; MCP is not yet wired.
- [x] `docs/en/ARCHITECTURE.md` and `docs/zh/ARCHITECTURE.md` updated where they currently misrepresent Desktop as build-time-core-only.
- [x] `python3 scripts/validate_tasks.py` passes for `tasks/TASK-2026-042-services-phase0-decisions/`.
- [x] `tasks/board.md` re-read after status change to confirm the entry actually reflects current state.

## Risks

- Risk: Phase 0 turns into "doc-only" without binding follow-up TASKs.
  - Impact: The plan stays a `todo/` artifact and the divergence keeps growing.
  - Mitigation: This TASK explicitly produces ADR-level decisions and lists the next concrete TASK (`services-phase1a-workspace`) under "Links".
- Risk: Entry/architecture doc edits drift between EN and ZH.
  - Impact: Violates `spec/docs-sync.md`; users read stale docs.
  - Mitigation: Treat EN+ZH as a single change unit; review checklist explicitly includes both files.
- Risk: D4 CI extension breaks on Desktop manifest.
  - Impact: PR blocked until `deny.toml` wrapper allow-list is correct.
  - Mitigation: Validate the command locally before adding to CI workflow.

## Validation Plan

- Required tests:
  - `python3 scripts/validate_tasks.py`
  - Local dry-run of new CI command (`cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`) once `deny.toml` is updated.
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo deny check bans` (workspace baseline, must still pass)
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans` (new Desktop coverage)
- Manual checks:
  - Re-read `docs/en/ENTRYPOINTS-AND-DOMAINS.md`, `docs/zh/ENTRYPOINTS-AND-DOMAINS.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md` to confirm Desktop description matches reality.
  - Re-read `tasks/board.md` after status change.

## Regression Scope

- Areas likely affected:
  - `docs/en/ENTRYPOINTS-AND-DOMAINS.md`
  - `docs/zh/ENTRYPOINTS-AND-DOMAINS.md`
  - `docs/en/ARCHITECTURE.md`
  - `docs/zh/ARCHITECTURE.md`
  - `deny.toml`
  - `.github/workflows/ci.yml` (or equivalent)
- Explicit non-goals:
  - No changes to runtime behavior, CLI commands, env vars, or Tauri commands.
  - No new Rust source files in this TASK.
  - No changes to `agent_loop`, `chat`, `evolution` write paths.

## Links

- Source TODO section: `todo/multi-entry-service-layer-refactor-plan.md` §4 (Phase 0 hard decisions)
- Related PRs/issues:
- Related docs:
  - `spec/architecture-boundaries.md`
  - `spec/docs-sync.md`
  - `spec/verification-integrity.md`
  - `crates/skilllite-assistant/README.md`
- Next TASK after this one: `services-phase1a-workspace` (to be created after Phase 0 closes)
