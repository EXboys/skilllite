# TASK Card

## Metadata

- Task ID: `TASK-2026-028`
- Title: Python SDK artifact HTTP + scenario pytest
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-08`
- Target milestone:

## Problem

Artifact HTTP had Rust tests only; integrators using Python needed a stdlib client and tests that mirror real user flows (save model output, multi-file run, isolation, bearer).

## Scope

- In scope:
  - `python-sdk/skilllite/artifacts.py`: `artifact_put`, `artifact_get`, `ArtifactHttpError`, `parse_listen_line`.
  - Main CLI `skilllite artifact-serve` (default `artifact_http`; bind gated by `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`) for local integration tests and embedders.
  - Pytest scenarios spawning the binary + HTTP roundtrip.
  - CI: Rust build step before `pytest`; docs (README, ARCHITECTURE EN/ZH, ENTRYPOINTS EN/ZH, CONTRIBUTING EN/ZH).
- Out of scope:
  - Live LLM calls; wiring ChatSession → artifact in Rust.

## Acceptance Criteria

- [x] Python API documented and exported from `skilllite` package; zero new PyPI runtime deps.
- [x] Integration tests pass when `target/{debug,release}/skilllite` exists (with `artifact-serve` subcommand).
- [x] `cargo test --workspace` still passes; CI workflow builds `skilllite` before pytest.

## Risks

- Risk: CI Rust step adds ~minutes.
  - Mitigation: Only `cargo build -p skilllite --bin skilllite`.

## Validation Plan

- `cargo build -p skilllite --bin skilllite`
- `cd python-sdk && ruff check . && ruff format --check . && mypy skilllite && pytest`
- `cargo test --workspace`

## Regression Scope

- `artifact-serve` is in the default `skilllite` binary but **bind** requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`; `skilllite-sandbox` omits the subcommand.

## Links

- OpenAPI: `docs/openapi/artifact-store-http-v1.yaml`
