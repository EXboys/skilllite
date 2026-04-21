# PRD

## Background

TASK-2026-042 locked Phase 0 decisions for the multi-entry service layer refactor. Decision D2 selected a new `skilllite-services` crate as the entry-neutral home for shared application services. To keep risk small and reversible, the bootstrap of that crate is intentionally separated from the first real service migration: this TASK only adds the empty crate, activates the pre-declared `cargo deny` boundary rule, and wires the workspace, while changing zero runtime behavior.

## Objective

Land an empty, compiling `skilllite-services` crate as a workspace member with all baseline quality gates green (`check`, `fmt`, `clippy -D warnings`, `cargo deny check bans` for both manifests, `validate_tasks.py`), so that follow-up Phase 1A TASKs can focus solely on service migration.

## Functional Requirements

- FR-1: Add `crates/skilllite-services/Cargo.toml` using workspace package fields and an empty `[dependencies]` section.
- FR-2: Add `crates/skilllite-services/src/lib.rs` with header rustdoc that summarizes purpose, current status, and boundary policy (per Phase 0 D1..D5), plus a single documented `BOOTSTRAP_PHASE` constant.
- FR-3: Do not modify root `Cargo.toml` or any other crate (the existing `crates/*` member glob auto-includes the new crate).
- FR-4: Keep zero observable runtime behavior changes (no new commands, env vars, Tauri commands, or MCP tools).

## Non-Functional Requirements

- Security: No security-relevant change.
- Performance: No runtime impact (no consumer depends on this crate yet).
- Compatibility: Adding a workspace member must not break any existing crate's `cargo check`/`build`/`test`.

## Constraints

- Technical:
  - Must comply with `spec/rust-conventions.md` (no `unsafe`, `forbid(unsafe_code)`, lints enabled).
  - Must comply with `spec/architecture-boundaries.md` (allowed wrappers only — entry layer; no domain-crate wrappers).
  - Must comply with `spec/task-artifact-language.md` (English task artifacts).
  - Must keep both `cargo deny check bans` invocations green.
- Timeline: This TASK is a prerequisite for `services-phase1a-workspace`. No external schedule binding.

## Success Metrics

- Metric: Number of acceptance criteria checked in `TASK.md`.
  - Baseline: 0 / 10.
  - Target: 10 / 10.
- Metric: New crates added to workspace.
  - Baseline: 0.
  - Target: 1 (`skilllite-services`).
- Metric: Build/lint/deny tooling exit codes.
  - Baseline: pass (root only).
  - Target: pass (workspace including new crate; both deny invocations).

## Rollout

- Rollout plan: Single PR adds the crate skeleton; no version bump; no migration; no behavior change.
- Rollback plan: Delete `crates/skilllite-services/` directory and revert this TASK; the `deny.toml` rule for `skilllite-services` becomes inert again (only emits `unused-wrapper` warnings, which are non-fatal).
