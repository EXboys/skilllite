# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-agent/src/llm/mod.rs`, `crates/skilllite-agent/src/llm/tests.rs`, `crates/skilllite-agent/src/prompt.rs`, `crates/skilllite-agent/src/types/string_utils.rs`.
- Current behavior: `format_api_error` extracts JSON messages safely, but its raw text fallback uses `&body[..200]`; skill reference prompt assembly uses `&content[..5000]`.

## Architecture Fit

- Layer boundaries involved: `skilllite-agent` internal formatting and prompt-building logic only.
- Interfaces to preserve: public LLM error formatting output shape and `get_skill_full_docs` return type.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: truncation limits remain byte-based ceilings; if the limit lands inside a multibyte character, output may be a few bytes shorter to preserve UTF-8 validity.

## Design Decisions

- Decision: reuse `crate::types::string_utils::safe_truncate`.
  - Rationale: the crate already has a UTF-8 boundary-safe helper used in related summary paths.
  - Alternatives considered: custom local loops at each call site.
  - Why rejected: duplication increases the chance of future divergence.

## Open Questions

- [x] Should docs change? No; behavior remains the same except avoiding crashes.
- [x] Are architecture boundaries affected? No; changes stay within `skilllite-agent`.
