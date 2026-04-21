# PRD

## Background

The repository now has four product entries (CLI, Desktop, MCP, Python SDK). Desktop has effectively become a first-class entry that directly consumes several core crates, but documentation, dependency policy (`deny.toml`), and architectural rules still describe it as a thin shell over the installed `skilllite` binary. Shared business flows are duplicated between `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/**` and `crates/skilllite-commands/src/**`. Before any Phase 1 extraction, the team needs to lock five hard decisions (D1..D5) so subsequent work does not constantly relitigate fundamentals.

## Objective

Produce a single, durable record of the Phase 0 decisions for the multi-entry service layer refactor, and update entry/architecture docs and dependency policy to match those decisions, without changing any runtime behavior.

## Functional Requirements

- FR-1: Lock D1 (Desktop is first-class) in `tasks/TASK-2026-042-services-phase0-decisions/CONTEXT.md` and reflect it in EN+ZH entry/architecture docs.
- FR-2: Lock D2 (new `skilllite-services` crate) in `CONTEXT.md`; do **not** create the crate in this TASK.
- FR-3: Lock D3 (async + per-crate `thiserror`) in `CONTEXT.md`; document adapter conversions.
- FR-4: Lock D4 (`cargo deny check bans` extended to Desktop manifest) and add the new CI command. Update `deny.toml` wrapper allow-list as needed for the future `skilllite-services` crate.
- FR-5: Lock D5 (serde-serializable, platform-neutral interfaces; Python SDK keeps IPC; MCP not wired) in `CONTEXT.md`.

## Non-Functional Requirements

- Security: No change to sandbox, security policy, or execution gating in this TASK.
- Performance: No runtime impact (docs + policy only).
- Compatibility: No CLI, env, Tauri command, or MCP tool changes.

## Constraints

- Technical:
  - Must comply with `spec/architecture-boundaries.md`, `spec/docs-sync.md`, `spec/verification-integrity.md`, `spec/task-artifact-language.md`, `spec/rust-conventions.md`.
  - All artifact files in this TASK folder must be in English.
- Timeline: This TASK should complete before any Phase 1A code work starts.

## Success Metrics

- Metric: Number of acceptance criteria in `TASK.md` checked off.
  - Baseline: 0 / 8.
  - Target: 8 / 8.
- Metric: `python3 scripts/validate_tasks.py` exit code.
  - Baseline: not yet run.
  - Target: 0 (pass).
- Metric: `cargo deny check bans` invocations covering both root workspace and Desktop manifest.
  - Baseline: 1 (root only).
  - Target: 2 (root + Desktop manifest).

## Rollout

- Rollout plan: Land docs + `deny.toml` + CI config in a single PR; no source code changes; no version bump required.
- Rollback plan: Revert the PR; no runtime artifacts are produced.
