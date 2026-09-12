# PRD

## Background

TASK-2026-024 introduced structured `RiskTier` so auto-approve paths can only
accept `low`. `SilentEventSink` was left as unconditional approve, creating a
bypass for ConfirmRequired tools on silent agent turns.

## Objective

- Silent/background sinks must never auto-approve `ConfirmRequired`.
- Low-risk confirmations may still auto-approve for silent memory-flush utility.

## Functional Requirements

- FR-1: `SilentEventSink::on_confirmation_request` returns true iff `risk_tier == Low`.
- FR-2: ConfirmRequired requests are denied (return false) without side effects.
- FR-3: Regression tests lock Low approve / ConfirmRequired deny.

## Non-Functional Requirements

- Security: Align with RiskTier contract and security non-negotiables.
- Performance: N/A (branch on enum).
- Compatibility: Background turns may cancel previously silent high-risk tool calls.

## Constraints

- Technical: Minimal change; no new confirmation channel for swarm in this task.
- Timeline: Same automation run.

## Success Metrics

- Metric: ConfirmRequired auto-approve via SilentEventSink
- Baseline: always true
- Target: always false for ConfirmRequired; true only for Low

## Rollout

- Rollout plan: merge via PR from critical-bug branch.
- Rollback plan: revert the SilentEventSink match change if unexpected breakage.
