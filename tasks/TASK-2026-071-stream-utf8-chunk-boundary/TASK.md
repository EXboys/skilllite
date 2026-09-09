# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Decode LLM SSE chunks on UTF-8 boundaries
- Status: `in_review`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-09-09`
- Target milestone:

## Problem

OpenAI-compatible and Claude SSE accumulators decode each `bytes_stream` chunk with `String::from_utf8_lossy` independently. When a multi-byte UTF-8 scalar (CJK / emoji) is split across HTTP body chunks, the incomplete prefix is replaced with U+FFFD and the continuation bytes are also corrupted. Those bytes are concatenated into streamed assistant text **and** tool-call `arguments`. A subsequent `write_file` / `search_replace` then persists the corrupted payload.

Concrete trigger: a streaming `write_file` whose JSON `content` contains `你好` (or any 3-byte CJK scalar) and whose HTTP frame boundary lands inside that scalar.

## Scope

- In scope:
  - Shared streaming UTF-8 decoder that holds incomplete trailing bytes until the next chunk
  - Wire the decoder into OpenAI and Claude SSE accumulators
  - Regression tests for split CJK and invalid mid-stream bytes
- Out of scope:
  - Changing tool-call merge / index logic
  - Transcript persist of `reasoning_content` (PR #154)
  - Non-stream HTTP JSON responses

## Acceptance Criteria

- [x] Incomplete UTF-8 at a chunk boundary is completed by the following chunk, not replaced with U+FFFD
- [x] Invalid mid-stream bytes still fail-open (U+FFFD) and do not stall the decoder
- [x] Remaining incomplete bytes at end-of-stream are flushed with lossy decode
- [x] OpenAI and Claude stream loops both use the shared decoder
- [x] `cargo test -p skilllite-agent` covers the new helper with a CJK split case
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass

## Risks

- Risk: Decoder holds bytes and delays a complete SSE line by one chunk
  - Impact: None for complete lines; only incomplete UTF-8 is held (1–3 bytes)
  - Mitigation: Flush pending bytes when the body stream ends
- Risk: Invalid UTF-8 in the middle of a chunk could loop
  - Impact: Hang
  - Mitigation: Advance past `error_len` and emit U+FFFD

## Validation Plan

- Required tests: unit tests on the shared decoder (CJK split, ASCII-only, invalid mid-stream, end flush)
- Commands to run:
  - `cargo test -p skilllite-agent --lib llm::`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks: N/A (no LLM key required)

## Regression Scope

- Areas likely affected: streaming chat completions (OpenAI-compatible + Claude)
- Explicit non-goals: non-stream `chat_completion` JSON parse; subprocess stdout lossy decode

## Links

- Source TODO section: daily critical-bug hunt 2026-09-09
- Related PRs/issues: #92–#96 (display truncation panics); #154 (reasoning persist)
- Related docs: N/A
