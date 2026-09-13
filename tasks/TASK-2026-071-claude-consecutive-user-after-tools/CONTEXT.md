# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/llm/claude.rs` (`convert_messages_for_claude`)
  - `crates/skilllite-agent/src/llm/tests.rs`
  - `crates/skilllite-agent/src/agent_loop/mod.rs` (injects user nudges after tool batches)
- Current behavior: tool rows flush to a `user` message; a following `ChatMessage` with `role=user` is pushed as another `user` message.

## Architecture Fit

- Layer boundaries involved: `skilllite-agent` LLM adapter only
- Interfaces to preserve: `convert_messages_for_claude(&[ChatMessage]) -> Result<(Option<String>, Vec<Value>)>`

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: legal Claude payloads stay legal; previously illegal consecutive-user payloads become legal

## Design Decisions

- Decision: coalesce consecutive same-role messages after conversion, and attach following user text to the flushed `tool_result` user message
  - Rationale: Anthropic requires one `user` message containing all `tool_result` blocks for the preceding `tool_use`, with any extra user text in that same message
  - Alternatives considered: stop injecting user nudges in the agent loop; rewrite nudges as assistant messages
  - Why rejected: nudges are valid OpenAI-protocol turns; the bug is Claude wire-format mapping, not planning control

## Open Questions

- [x] Does chat mode (planning off) hit this? Only via rare clarify/budget paths; run mode + Claude + `complete_task` is the default trigger.
- [x] Should OpenAI conversion change? No.
