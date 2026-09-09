# PRD

## Background

SkillLite streams LLM tokens and tool-call argument fragments over SSE. The HTTP body is consumed as raw byte chunks. Chinese and emoji tool arguments are common on this product. Independently lossy-decoding each chunk silently corrupts files the agent writes.

## Objective

Streaming assembly of assistant text and tool-call arguments must preserve every complete UTF-8 scalar even when a scalar is split across consecutive body chunks.

## Functional Requirements

- FR-1: Hold back 1–3 trailing bytes that are an incomplete UTF-8 sequence until the next chunk arrives.
- FR-2: When the next chunk completes the sequence, emit the original character (not U+FFFD).
- FR-3: Truly invalid bytes (not just incomplete) are replaced with U+FFFD and the stream continues.
- FR-4: End of stream flushes any leftover pending bytes with lossy decode so a truncated body cannot drop the last line forever.

## Non-Functional Requirements

- Security: No change to path/sandbox policy.
- Performance: O(chunk) with at most 3 bytes of pending state.
- Compatibility: Non-stream responses unchanged. ASCII streams unchanged.

## Constraints

- Technical: Keep the helper inside `skilllite-agent` LLM module; no new crate.
- Timeline: Same-day critical-bug hunt.

## Success Metrics

- Metric: CJK scalar split across two chunks round-trips
- Baseline: `from_utf8_lossy` per chunk emits U+FFFD
- Target: helper concatenates to the original string

## Rollout

- Rollout plan: merge via PR; no feature flag
- Rollback plan: revert the commit
