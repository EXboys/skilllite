# TASK Card

## Metadata

- Task ID: `TASK-2026-029`
- Title: Fix assistant transcript tool_call_id linkage
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors: `Cursor`
- Created: `2026-04-09`
- Target milestone: `desktop transcript stability`

## Problem

`skilllite-assistant` restores transcript tool results by guessing the most recent
`read_file` call path instead of using the persisted `tool_call_id` linkage that
already exists in executor transcripts. This can attach the wrong `sourcePath`
to restored `read_file` results after multi-tool turns, breaking "open in IDE"
and file preview fidelity.

## Scope

- In scope:
  - Expose `tool_call_id` from the assistant Tauri transcript DTO.
  - Rebuild restored `read_file` result linkage in the desktop frontend by matching `tool_call_id`.
  - Add a focused regression test for transcript restoration.
  - Propagate `tool_call_id` through the live agent-rpc event stream into `useChatEvents`.
  - Unify live and restored desktop tool message linkage around the same `tool_call_id` model.
- Out of scope:
  - Broader transcript/UI refactors unrelated to `read_file` result linkage.

## Acceptance Criteria

- [x] Tauri transcript payload includes `tool_call_id` for restored tool call/result rows.
- [x] Desktop transcript reload derives `read_file` `sourcePath` via matching `tool_call_id`, not adjacency.
- [x] Regression coverage proves restored `tool_result` rows keep the correct `tool_call_id` chain.
- [x] Agent RPC `tool_call` / `tool_result` live events include `tool_call_id`.
- [x] `useChatEvents` links live `read_file` results via `tool_call_id` and uses it in dedupe identity.

## Risks

- Risk:
  - Impact: Transcript compatibility regressions could hide tool rows or break older history rendering.
  - Mitigation: Keep `tool_call_id` optional in the DTO and only use the new path when ids are present.

## Validation Plan

- Required tests: targeted Rust transcript restoration test, desktop type/build checks for touched files.
- Commands to run: `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`, `npm run build`
- Manual checks: verify restored `read_file` results still render previews and preserve the correct source path.

## Regression Scope

- Areas likely affected: `skilllite-assistant` transcript restoration, chat history previews, file-open affordances.
- Explicit non-goals: changing runtime event payload shape or non-transcript tool rendering.

## Links

- Source TODO section: user-requested fix from architecture/code review follow-up
- Related PRs/issues: N/A
- Related docs: N/A
