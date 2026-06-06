# PRD

## Background

Deep bug-finding found a crash-class truncation bug in the agent LLM error path. When an LLM provider or gateway returns a long, non-JSON error body containing multibyte text, the current fallback summary slices the body by byte index and can panic.

## Objective

LLM API error summaries and nearby prompt reference truncation should never split UTF-8 code points. Users should receive a normal formatted API error rather than a process panic.

## Functional Requirements

- FR-1: Preserve existing JSON error-message extraction behavior.
- FR-2: Preserve existing raw-body byte ceilings while adjusting the slice endpoint to a valid UTF-8 boundary.
- FR-3: Cover long non-ASCII fallback bodies with regression tests.

## Non-Functional Requirements

- Security: no new data exposure; keep summaries bounded.
- Performance: truncation remains O(limit adjustment) with no allocation beyond the resulting summary.
- Compatibility: output remains a string with the same friendly hints and ellipsis behavior.

## Constraints

- Technical: use existing string helper utilities rather than adding dependencies.
- Timeline: N/A for autonomous execution.

## Success Metrics

- Metric: non-ASCII long fallback error bodies do not panic.
- Baseline: `extract_error_detail` can panic at `&body[..200]`.
- Target: regression test exercises the exact formatting path successfully.

## Rollout

- Rollout plan: ship as a minimal Rust bug fix with tests.
- Rollback plan: revert the small helper usage and tests if an unexpected regression appears.
