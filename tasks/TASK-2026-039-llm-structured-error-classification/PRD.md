# PRD

## Background

MVP-A fallback improves reliability, but its retry decisions are still heuristic. A durable routing system needs a stable error taxonomy so automatic switching happens only on genuinely retryable failures.

## Objective

Introduce structured LLM error classification suitable for routing decisions, reducing false-positive and false-negative fallback behavior.

## Functional Requirements

- FR-1: The system distinguishes retryable transient failures from non-retryable configuration/auth/request failures.
- FR-2: The assistant fallback logic can consume a typed classification rather than parse provider message text.
- FR-3: The classification remains backward compatible where raw string errors still exist temporarily.

## Non-Functional Requirements

- Security: Auth/config errors must not silently degrade into retries against unrelated profiles.
- Performance: Error classification should add negligible overhead compared with the failing request itself.
- Compatibility: Existing invoke callers should keep working while the routing-aware callers adopt the structured path.

## Constraints

- Technical: Likely spans Rust bridge + assistant TypeScript.
- Timeline: Small focused follow-up after MVP-A.

## Success Metrics

- Metric: Retry decisions align with actual error semantics.
- Baseline: Raw substring heuristic.
- Target: Typed routing classification in production path.

## Rollout

- Rollout plan: Implement typed envelope, wire non-streaming fallback callers first, retain raw-string compatibility until full migration.
- Rollback plan: Fall back to the current heuristic helper while keeping the new type internal.
