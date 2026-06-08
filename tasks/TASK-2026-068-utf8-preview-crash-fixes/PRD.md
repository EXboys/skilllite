# PRD

## Background

The latest critical-bug audits fixed UTF-8 unsafe truncation in selected evolution and LLM error
paths. Follow-up inspection found adjacent code that still slices user/model/provider text by byte
index. These paths can crash on valid UTF-8 when long CJK or emoji content reaches a preview cap.

## Objective

Prevent crash-class UTF-8 boundary panics in the identified evolution status and agent preview
paths while preserving existing command/error behavior.

## Functional Requirements

- FR-1: Evolution status human output must display shortened event reasons without byte-slicing
  arbitrary UTF-8.
- FR-2: Planning-control validation must return a structured tool error for malformed multibyte
  `tasks` strings.
- FR-3: Embedding response validation must return an error for unexpected multibyte JSON response
  bodies instead of panicking while building the preview.

## Non-Functional Requirements

- Security: No new sandbox, auth, or policy surface.
- Performance: Truncation remains bounded and allocation impact is negligible on error/display paths.
- Compatibility: Existing CLI flags, JSON schemas, and error semantics are preserved.

## Constraints

- Technical: Reuse existing Unicode-safe truncation helpers where available; avoid broad utility
  migrations unrelated to the concrete crash paths.
- Timeline: N/A for autonomous execution; keep the change small and reviewable.

## Success Metrics

- Metric: Non-ASCII regression tests for the fixed paths.
- Baseline: Byte slicing can panic at fixed byte caps.
- Target: Tests and manual CLI reproduction complete without panic.

## Rollout

- Rollout plan: Ship as a small bug-fix PR on the assigned branch.
- Rollback plan: Revert the commit if regressions appear; no data migration is involved.
