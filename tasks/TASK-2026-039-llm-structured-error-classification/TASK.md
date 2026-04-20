# TASK Card

## Metadata

- Task ID: `TASK-2026-039`
- Title: Structured LLM error classification for routing
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

The current fallback helper classifies retryable errors by substring-matching raw Tauri error messages (e.g. `429`, `timeout`, `network`). This works for MVP-A, but can silently misclassify auth/configuration errors as retryable or fail to detect transient provider errors when wording changes.

## Scope

- In scope:
  - Define a stable assistant-facing error taxonomy for routing decisions (`retryable`, `non_retryable`, and specific reasons like `rate_limited`, `provider_unavailable`, `network_timeout`, `auth_invalid`, `bad_request`, `model_not_found`).
  - Decide where the structured classification should originate (preferred: Rust / bridge layer) and how the assistant consumes it.
  - Replace or minimize raw string heuristics in fallback routing paths.
  - Document compatibility strategy for older/raw error paths if the bridge still emits plain strings in some cases.
- Out of scope:
  - Heuristic complexity-based routing.
  - Cost / health scoring.
  - Streaming `agent` fallback.

## Acceptance Criteria

- [x] A concrete typed error shape / enum for routing is defined and documented.
- [x] Retryable vs non-retryable decisions no longer depend primarily on message substring matching.
- [x] Assistant fallback logic consumes the structured classification for non-streaming routes.
- [x] Validation evidence covers at least one retryable and one non-retryable path.

## Risks

- Risk: Cross-layer error refactor may touch Rust, Tauri bridge, and assistant TS simultaneously.
  - Impact: Wider regression surface if serialized shapes drift.
  - Mitigation: Keep a thin compatibility layer and verify actual IPC payloads.

## Validation Plan

- Required tests: assistant build; Rust formatting/test suite; workspace clippy noted below.
- Commands to run:
  - `cargo fmt --all --check`
  - `cd crates/skilllite-assistant && npm run build`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
- Manual checks: trigger a known retryable failure and a known auth/configuration failure, verify only the retryable one falls back.

## Regression Scope

- Areas likely affected: Tauri error serialization, assistant fallback helper, any invoke sites using scenario fallback.
- Explicit non-goals: making routing smarter by task complexity.

## Links

- Source TODO section: `todo/assistant-auto-llm-routing-plan.md` §10.2
- Related PRs/issues:
- Related docs: `crates/skilllite-assistant/README.md`, `todo/assistant-auto-llm-routing-plan.md`
