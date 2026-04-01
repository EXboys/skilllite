# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/rpc.rs`
  - `crates/skilllite-agent/src/types/event_sink.rs`
- Current behavior:
  - RPC sink emits every `on_tool_result` call directly.
  - No same-turn dedupe guard exists.

## Architecture Fit

- Layer boundaries involved:
  - Agent RPC output layer only.
- Interfaces to preserve:
  - Existing RPC event names/payloads.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Additive internal dedupe only; protocol remains identical.

## Design Decisions

- Decision:
  - Add in-sink dedupe set keyed by turn and hashed result signature.
  - Rationale:
    - Prevent duplicate UI display without altering execution semantics.
  - Alternatives considered:
    - Client-side dedupe in each SDK/UI.
  - Why rejected:
    - Would duplicate logic across consumers and leave protocol-level noise.

## Open Questions

- [ ] Should dedupe logic be generalized to other event types (for example `text_chunk`)?
- [ ] Should dedupe be configurable via env for debugging?
