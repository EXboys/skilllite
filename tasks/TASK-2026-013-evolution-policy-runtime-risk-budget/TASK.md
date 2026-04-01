# TASK Card

## Metadata

- Task ID: `TASK-2026-013`
- Title: Evolution: policy runtime and risk budget gating
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone: `P7-D (1-2 weeks)`

## Problem

The evolution coordinator currently has only coarse execution gates (`shadow_mode`, low-risk auto-run),
which is not enough to explain policy decisions in high-risk scenarios or to cap daily risk exposure.
This weakens trust and makes rollout harder in production-like environments.

## Scope

- In scope:
- Add a policy runtime evaluator in evolution coordinator that returns structured decision (`allow` / `ask` / `deny`) with reason chain.
- Add a daily risk budget strategy per risk level for auto execution gating.
- Persist policy decision reasons into backlog notes for auditability.
- Keep existing force-run behavior and default secure posture.
- Update env key constants and EN/ZH env reference docs.
- Out of scope:
- Multi-step human approval workflow UI.
- New dashboard/analytics surface.
- Re-designing evolution proposal generation heuristics.

## Acceptance Criteria

- [x] Coordinator policy runtime produces deterministic decisions with reason chain.
- [x] Daily risk budgets are enforced for auto execution and covered by tests (including exhaustion path).
- [x] Default behavior remains non-more-permissive (shadow mode + medium/high not auto-executed by default).
- [x] `skilllite evolution run` force path still bypasses policy runtime auto gates.
- [x] EN/ZH env docs are updated for new policy/budget variables.

## Risks

- Risk: Budget defaults too strict could reduce automation throughput.
  - Impact: Fewer auto-executed proposals than expected.
  - Mitigation: Keep defaults aligned with safe rollout, provide explicit env overrides.
- Risk: Policy decision note may become hard to read if too verbose.
  - Impact: Lower operator usability during incident review.
  - Mitigation: Keep reason chain concise and deterministic.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-evolution`
  - `cargo test -p skilllite`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks:
  - Verify backlog note contains policy runtime decision and reason chain.
  - Verify budget-exhausted low-risk proposal is queued instead of executed.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-evolution/src/lib.rs` coordinator decision path.
  - Env key declarations and env reference docs.
- Explicit non-goals:
  - No change to sandbox level semantics (L1/L2/L3).
  - No change to skill pending-confirm flow.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section `15.5 P7-D`
- Related PRs/issues:
- Related docs:
  - `docs/en/ENV_REFERENCE.md`
  - `docs/zh/ENV_REFERENCE.md`
