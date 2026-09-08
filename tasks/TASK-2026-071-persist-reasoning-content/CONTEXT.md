# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-executor/src/transcript.rs`
  - `crates/skilllite-agent/src/chat_session.rs`
  - `crates/skilllite-agent/src/llm/openai.rs`
  - `crates/skilllite-agent/src/types/chat.rs`
- Current behavior:
  - Agent loop stores `reasoning_content` on in-memory `ChatMessage`.
  - `append_assistant_message` writes `content` + `llm_usage` only.
  - `transcript_entry_to_message` hard-codes `reasoning_content: None`.

## Architecture Fit

- Layer boundaries involved: executor transcript schema; agent chat session persist/reload; LLM request mapping.
- Interfaces to preserve: existing `TranscriptEntry::Message` tag and required fields.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: optional field with `serde(default)`.

## Design Decisions

- Decision: Store `reasoning_content` on the final assistant `Message` row, sourced from the last assistant message in `AgentResult.messages`.
  - Rationale: That is the row `read_history` replays between user turns; DeepSeek requires the field whenever `tools` are present.
  - Alternatives considered: Persist every intermediate tool-call assistant row; rewrite full tool history.
  - Why rejected: Intermediate tool rows already live in memory for the current turn; broader persist is out of scope.

## Open Questions

- [x] Does DeepSeek require the field without tools? Docs say no; SkillLite agent requests still send tools, so the field is required on follow-up turns.
