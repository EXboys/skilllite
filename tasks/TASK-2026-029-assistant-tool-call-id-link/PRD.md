# PRD

## Background

The desktop assistant already stores stable `tool_call_id` values in transcript
rows, but the restore path only exposes tool name/content and then guesses the
matching `read_file` call by adjacency. This causes incorrect file metadata
association after multi-tool turns and weakens transcript determinism.

## Objective

Restore transcript tool rows with enough identity to reconstruct the original
tool-call/result relationship, and use that identity to recover the correct
`read_file` source path in the UI.

## Functional Requirements

- FR-1: The Tauri transcript bridge must serialize `tool_call_id` for restored `tool_call` and `tool_result` rows when present.
- FR-2: The desktop transcript reload flow must derive `read_file` `sourcePath` from matching `tool_call_id`, not from the previous row.
- FR-3: The live agent-rpc `tool_call` and `tool_result` events must include `tool_call_id` so the desktop can apply the same linkage logic during streaming sessions.

## Non-Functional Requirements

- Security:
  - No expansion of file access scope; this is metadata-only restoration.
- Performance:
  - The new linkage must remain O(n) over restored transcript rows.
- Compatibility:
  - Older transcripts without `tool_call_id` must continue to load without errors.

## Constraints

- Technical:
  - Keep the change scoped to transcript restoration; do not redesign live RPC events in this task.
- Timeline:
  - Single implementation pass with one focused regression test.

## Success Metrics

- Metric: `read_file` restored rows carry the correct `sourcePath` when multiple tool calls exist in the same transcript segment.
- Baseline: Path association is inferred from the most recent `read_file` call.
- Target: Path association uses explicit `tool_call_id` linkage whenever the transcript provides it.

## Rollout

- Rollout plan: Ship with optional DTO field so older history remains readable.
- Rollback plan: Revert the DTO field and frontend matching logic together if transcript rendering regresses.
