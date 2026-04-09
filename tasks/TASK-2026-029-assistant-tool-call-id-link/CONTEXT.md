# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-executor/src/transcript.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/transcript.rs`
  - `crates/skilllite-assistant/src/components/ChatView.tsx`
- Current behavior:
  - Executor transcript rows already persist `tool_call_id` on both tool call and tool result entries.
  - The assistant Tauri bridge drops that field when restoring transcript rows for the frontend.
  - `ChatView` infers `read_file` result metadata from the most recent preceding `tool_call`.
  - The live agent-rpc event stream also omits `tool_call_id`, so `useChatEvents` still relies on adjacency and content-only dedupe.

## Architecture Fit

- Layer boundaries involved:
  - Executor transcript persistence -> assistant Tauri bridge DTO -> React transcript restore UI.
- Interfaces to preserve:
  - Existing transcript row shape for callers that do not inspect `tool_call_id`.
  - `ChatMessage` rendering contracts for existing tool result bubbles.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - `tool_call_id` must be optional because old transcript files and existing callers may omit it.

## Design Decisions

- Decision:
  - Rationale: Preserve the executor's structured linkage end-to-end and reconstruct `read_file` metadata deterministically in the UI.
  - Alternatives considered:
    - Keep adjacency-based matching.
  - Why rejected:
    - Adjacency-based matching is the bug.
    - N/A after scope expansion; live and restored paths should now converge on one model.

## Open Questions

- [x] Should the live `tool_call` / `tool_result` event payload also expose `tool_call_id` in a follow-up task?
- [x] Should restored frontend messages keep `toolCallId` on the `ChatMessage` model for future tooling?
