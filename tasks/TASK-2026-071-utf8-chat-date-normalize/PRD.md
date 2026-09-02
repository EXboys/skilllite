# PRD

## Background

The scheduled high-severity sweep found a crash in builtin chat tools. When an LLM (common for Chinese users) passes a localized month such as `2026年9` or `2026-9月`, `normalize_date` sees an 8-byte string and byte-slices it. Rust panics on a mid-character index, and the agent loop does not catch unwinds, so the chat/CLI process dies instead of returning "no history".

## Objective

Chat history and plan date arguments must never panic on non-ASCII input. Compact `YYYYMMDD` / `YYYY-MM-DD` digit dates keep the existing normalized filename form.

## Functional Requirements

- FR-1: Compact 8-digit dates (`20260902`, `2026-09-02`) still normalize to `2026-09-02`.
- FR-2: Any other date string, including 8-byte CJK forms, is left unchanged and must not panic.
- FR-3: `chat_history` and `chat_plan` return a normal `Ok` tool result for those CJK dates.

## Non-Functional Requirements

- Security: no new path or session-key behavior.
- Performance: O(n) ASCII-digit scan of an 8-byte string.
- Compatibility: valid ISO compact dates keep the same lookup key.

## Constraints

- Technical: stay inside `skilllite-agent` builtin chat tools; no new dependencies.
- Timeline: N/A for autonomous execution.

## Success Metrics

- Metric: localized 8-byte dates do not panic.
- Baseline: `normalize_date("2026年9")` panics at `&s[4..6]`.
- Target: regression tests cover ASCII compact dates and CJK 8-byte dates.

## Rollout

- Rollout plan: minimal Rust fix plus tests.
- Rollback plan: revert the helper and tests if a regression appears.
