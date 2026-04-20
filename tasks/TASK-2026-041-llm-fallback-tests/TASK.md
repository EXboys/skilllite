# TASK Card

## Metadata

- Task ID: `TASK-2026-041`
- Title: LLM scenario fallback logic tests
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-20`
- Target milestone:

## Problem

Scenario fallback now includes candidate-chain building, retryable-error checks, cooldown, and multiple foreground/non-streaming call-site integrations. Without targeted tests, regressions in these branches could silently break the reliability guarantees introduced by MVP-A.

## Scope

- In scope:
  - Add focused tests for fallback helper behavior (candidate ordering, duplicate filtering, retryable/non-retryable branching, cooldown skipping, multi-fallback progression).
  - Add or define test seams for helper functions that currently rely on time / process-local state.
  - Validate at least one caller-level integration path if practical.
- Out of scope:
  - Full end-to-end provider integration tests.
  - Streaming `agent` fallback tests (not implemented).
  - Health-score / complexity routing tests.

## Acceptance Criteria

- [x] Tests prove a retryable primary failure switches to the next fallback.
- [x] Tests prove non-retryable errors do not trigger switching.
- [x] Tests prove cooldown skips a recently failed profile.
- [x] Tests cover duplicate / missing profile handling in candidate building.
- [x] Test suite / targeted test command passes and is documented in the task evidence.

## Risks

- Risk: Current helper shape may be hard to test because of direct time access and module-level state.
  - Impact: Tests become brittle or rely too much on implementation details.
  - Mitigation: Reuse the existing `resetLlmFallbackCooldown()` seam and keep the test harness focused on the helper boundary.

## Validation Plan

- Required tests: focused fallback logic tests + assistant build + workspace baseline checks.
- Commands to run:
  - `cd crates/skilllite-assistant && npm run test:llm-fallback`
  - `cd crates/skilllite-assistant && npm run build`
  - `cargo fmt --all --check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
- Manual checks: optional spot-check by forcing a primary failure in a non-streaming scenario.

## Regression Scope

- Areas likely affected: `llmScenarioFallback.ts`, fallback toast wrapper, non-streaming invoke wrappers.
- Explicit non-goals: changing routing policy or adding new scenarios.

## Links

- Source TODO section: `todo/assistant-auto-llm-routing-plan.md` §10.2
- Related PRs/issues:
- Related docs: `todo/assistant-auto-llm-routing-plan.md`
