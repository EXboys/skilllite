# TASK Card

## Metadata

- Task ID: `TASK-2026-027`
- Title: Unify artifact impls into skilllite-artifact
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-08`
- Target milestone:

## Problem

`LocalDirArtifactStore` lived in `skilllite-agent` while HTTP lived in `skilllite-artifact-http`, which felt scattered after introducing a dedicated crate.

## Scope

- In scope:
  - Single crate `skilllite-artifact` with features `local`, `server`, `client` (defaults: all three for full tests).
  - Agent depends on `skilllite-artifact` with `default-features = false, features = ["local"]` to avoid Axum/reqwest in the default agent build.
  - Remove `skilllite-artifact-http` and `skilllite-agent/src/artifact_store.rs`.
  - Update EN/ZH ARCHITECTURE, OpenAPI crate reference, lockfile.
- Out of scope:
  - *(Superseded by `TASK-2026-028`.)* CLI `artifact-serve` was originally out of scope for 027 but shipped in 028 with bind gating.

## Acceptance Criteria

- [x] `LocalDirArtifactStore` + HTTP server/client live under `crates/skilllite-artifact/`.
- [x] Default agent build does not compile HTTP dependencies.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` pass.
- [x] Docs and OpenAPI reference updated.

## Risks

- Risk: Downstream users imported `skilllite-artifact-http` by crate name.
  - Impact: Breakage for external dependents (crate was new).
  - Mitigation: Document rename in ARCHITECTURE; publish note in changelog if applicable.

## Validation Plan

- Commands: `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --all -- --check`, `python3 scripts/validate_tasks.py`.

## Regression Scope

- `skilllite-agent` artifact default path behavior unchanged (same layout under `data_root`).

## Links

- Supersedes crate layout from `TASK-2026-026-artifact-http-api`.
