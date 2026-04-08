# TASK Card

## Metadata

- Task ID: `TASK-2026-026`
- Title: HTTP artifact store API + OpenAPI
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-08`
- Target milestone:

## Problem

Integrators asked for a language-neutral way to read/write run-scoped artifacts across processes or machines. The core already exposes `ArtifactStore`; we needed an HTTP surface and a documented contract.

## Scope

- In scope:
  - New crate `skilllite-artifact-http` depending only on `skilllite-core`.
  - Axum router: `GET`/`PUT` `/v1/runs/{run_id}/artifacts?key=...` with optional bearer auth.
  - Blocking `HttpArtifactStore` client (`reqwest::blocking`) implementing `ArtifactStore`.
  - OpenAPI v1 under `docs/openapi/artifact-store-http-v1.yaml`.
  - EN/ZH architecture doc updates (crate + link).
- Out of scope:
  - Wiring the HTTP server into the main `skilllite` CLI (embedders start their own listener).
  - Python SDK changes (follow-up).
  - Streaming uploads, listing keys, TLS termination (use a reverse proxy).

## Acceptance Criteria

- [x] HTTP semantics match `ArtifactStore` get/put for bytes.
- [x] OpenAPI file describes paths, query `key`, status codes, optional bearer, error JSON shape.
- [x] Unit/integration tests cover server roundtrip, auth rejection, and client against wiremock.
- [x] `cargo test --workspace` and `cargo clippy --workspace --all-targets` pass.
- [x] `docs/en/ARCHITECTURE.md` and `docs/zh/ARCHITECTURE.md` updated consistently.

## Risks

- Risk: `reqwest::blocking` inside async runtimes can block the executor.
  - Impact: Latency or deadlock if misused.
  - Mitigation: Document; server uses `spawn_blocking` for store I/O; clients in async code should call from `spawn_blocking` or use an async client in a future task.

## Validation Plan

- Required tests: crate unit tests + workspace suite.
- Commands to run: `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `python3 scripts/validate_tasks.py`.
- Manual checks: None required.

## Regression Scope

- Areas likely affected: New crate + lockfile + docs only.
- Explicit non-goals: No change to default agent behavior.

## Links

- OpenAPI: `docs/openapi/artifact-store-http-v1.yaml`
- Prior: `TASK-2026-025-run-scoped-artifact-store`
