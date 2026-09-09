# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/llm/openai.rs` (`accumulate_openai_stream`)
  - `crates/skilllite-agent/src/llm/claude.rs` (`accumulate_claude_stream`)
  - `crates/skilllite-agent/src/llm/mod.rs`
- Current behavior: `buffer.push_str(&String::from_utf8_lossy(&chunk))` on every `bytes_stream` item.

## Architecture Fit

- Layer boundaries involved: agent LLM client only (no crate graph change)
- Interfaces to preserve: `LlmClient` public methods and `ChatCompletionResponse` shape

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: streamed text that previously contained U+FFFD from split CJK will now contain the original characters. Callers that compared against replacement characters would change; none exist.

## Design Decisions

- Decision: Shared `take_complete_utf8` / `flush_pending_utf8` in `llm/mod.rs` using `std::str::from_utf8` + `Utf8Error::valid_up_to` / `error_len`
  - Rationale: Standard library already distinguishes incomplete-at-end (`error_len == None`) from invalid sequences
  - Alternatives considered: `encoding_rs` streaming decoder; keep a `String` and only decode with lossy
  - Why rejected: extra dependency; lossy-per-chunk is the bug

## Open Questions

- [x] Should leftover pending bytes at EOS be dropped or lossy-flushed? Flushed, so a truncated last SSE line can still be parsed when it has a newline after flush.
