# PRD

## Background

The planning agent loop injects a follow-up `ChatMessage::user(...)` immediately after tool results when:

- pending tasks remain and free-form assistant text was suppressed
- a per-task tool-call depth limit was hit
- all tasks completed without a substantial assistant/tool summary (`CLOSING_SUMMARY_USER_MESSAGE`)

Those in-memory rows are correct for the OpenAI tool protocol. The Claude converter already batches `role=tool` rows into one `user` message with `tool_result` blocks, but then appends the injected user text as a second `user` message. Official Anthropic Messages API rejects consecutive same-role messages.

## Objective

Claude native runs that finish a tool batch and then inject a user nudge must produce a legal Messages payload (alternating roles) so the next completion can succeed.

## Functional Requirements

- FR-1: After conversion, no two adjacent Claude messages share the same `role` (`user` or `assistant`).
- FR-2: `tool_result` blocks remain in the `user` message that follows the matching `tool_use` assistant message.
- FR-3: Injected user text (including CJK) is preserved as a `text` block on that same `user` message, after the `tool_result` blocks.

## Non-Functional Requirements

- Security: no change to auth, sandbox, or path handling
- Performance: O(n) coalesce over the converted message list
- Compatibility: OpenAI conversion unchanged; Claude payloads that already alternate are unchanged

## Constraints

- Technical: keep the fix inside `convert_messages_for_claude`; do not rewrite the agent loop
- Timeline: same-day critical-bug sweep

## Success Metrics

- Metric: closing-summary fixture converts to 3 messages (`user`, `assistant`, `user`) not 4
- Baseline: current converter emits 4 messages with two trailing `user` roles
- Target: one trailing `user` with `tool_result` + `text`

## Rollout

- Rollout plan: merge via PR; no flag
- Rollback plan: revert the conversion helper
