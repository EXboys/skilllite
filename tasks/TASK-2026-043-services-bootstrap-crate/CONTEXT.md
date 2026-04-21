# Technical Context

## Current State

- Relevant crates/files (post-TASK-2026-042):
  - `Cargo.toml` (root workspace; `members = ["skilllite", "crates/*"]`, `exclude = ["crates/skilllite-assistant", "crates/crates"]`).
  - `deny.toml` (Phase 0 D2: pre-declares the `skilllite-services` rule with wrappers `skilllite`, `skilllite-commands`, `skilllite-assistant`; rule is inert until a consumer appears).
  - `.github/workflows/ci.yml` (Phase 0 D4: runs `cargo deny check bans` against both root and Desktop manifest).
  - `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md` (Phase 0: dependency-graph section already mentions the future `skilllite-services` layer).
- Current behavior:
  - No `skilllite-services` crate exists yet.
  - Both `cargo deny check bans` invocations pass with `unused-wrapper` warnings for the pre-declared `skilllite-services` rule.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-services` sits between entry crates (`skilllite`, `skilllite-commands`, `skilllite-assistant`) and domain crates (`skilllite-{core,fs,sandbox,executor,agent,evolution,artifact,swarm}`).
  - Per Phase 0 D2, only entry-layer crates may depend on `skilllite-services`; domain crates must not.
  - This TASK does not yet add any reverse dependency, but the boundary rule is now actively checked once the crate exists in the graph.
- Interfaces to preserve:
  - All existing CLI subcommands and their stdout/stderr contract.
  - All existing Tauri commands exposed by `skilllite_bridge`.
  - All existing MCP tool schemas.
  - All existing Python SDK subprocess/IPC behavior.

## Dependency and Compatibility

- New dependencies:
  - None at the crate level. `[dependencies]` is intentionally empty in this TASK.
  - Workspace inherits via `version.workspace`, `edition.workspace`, etc.; no new entries in `[workspace.dependencies]`.
- Backward compatibility notes:
  - Adding a workspace member is additive; existing crates' lockfile resolution and feature graph are unaffected (verified by `cargo check --workspace`).
  - Once a future PR makes any entry crate depend on `skilllite-services`, that's the point at which the deny rule actually gates compilation; this TASK does not introduce any such dependency.

## Design Decisions

- Decision — Empty `[dependencies]` block.
  - Rationale: Avoids pulling `tokio` / `serde` / `thiserror` into a placeholder crate; respects the principle "first PR adds the crate, second PR adds the first real service".
  - Alternatives considered: Pre-add `tokio`, `serde`, `thiserror` to anticipate Phase 1A.
  - Why rejected: Would make the bootstrap diff larger and would emit clippy/unused-dep warnings; harder to reason about as a no-op landing.

- Decision — `BOOTSTRAP_PHASE: &str` placeholder constant.
  - Rationale: Gives downstream code a trivial way to confirm the crate is wired in (`use skilllite_services::BOOTSTRAP_PHASE`), and prevents an empty `lib.rs` from being optimized away.
  - Alternatives considered: Truly empty `lib.rs`.
  - Why rejected: An empty crate would still compile, but the placeholder constant doubles as documentation and an easy smoke-test target.

- Decision — Lints `forbid(unsafe_code)`, `deny(rust_2018_idioms)`, `warn(missing_docs)`.
  - Rationale: Aligns with `spec/rust-conventions.md` defaults observed in other workspace crates; enforces no `unsafe` from the start; documentation discipline before any service lands.
  - Alternatives considered: Defer lint configuration until Phase 1A.
  - Why rejected: Cheaper to set lint baseline now; harder to retrofit once code accumulates.

- Decision — Do not modify root `Cargo.toml`.
  - Rationale: Existing `crates/*` glob already includes the new directory; modifying root introduces unnecessary churn.
  - Alternatives considered: Explicitly list `crates/skilllite-services` in `members`.
  - Why rejected: Redundant; would inconsistently pin one crate while leaving others under the glob.

## Open Questions

- [ ] When the first real service lands, should `BOOTSTRAP_PHASE` be removed immediately or kept until the end of Phase 1B for diagnostic convenience? (Defer to Phase 1A TASK author.)
- [ ] Should `skilllite-services` enable `lints.workspace = true` once the workspace introduces a `[workspace.lints]` table? (Out of scope for this TASK; raise as a separate cleanup if/when applicable.)
