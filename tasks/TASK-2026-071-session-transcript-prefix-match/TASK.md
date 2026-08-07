# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Fix session transcript/plan prefix matching
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-08-07`
- Target milestone:

## Problem

`list_transcript_files` (and `list_plan_files`) matched session files with a bare `starts_with(session_key)`. Session keys that are prefixes of sibling keys (`s1` vs `s10`, `schedule-1` vs `schedule-10`) caused cross-session transcript/plan reads into agent history and tools.

## Scope

- In scope:
  - Bound dated transcript/plan filename matching to `{session_key}-` (plus exact legacy `{session_key}.jsonl` / stem equality).
  - Align desktop recent-plan candidate filter with the same boundary rule.
  - Add regression tests for prefix-sibling keys.
- Out of scope:
  - Evolution backlog `dedupe_key` UNIQUE reuse after `executed` (tracked as follow-up finding).
  - Concurrent `sessions.json` last-writer-wins races.
  - Open path-escape PRs (#89, #123–#134).

## Acceptance Criteria

- [x] `list_transcript_files("s1")` does not include `s10-*.jsonl`.
- [x] `list_plan_files("s1")` does not include `s10-*` plans.
- [x] Legacy `{session_key}.jsonl` / exact stem plans still listed.
- [x] Regression tests cover prefix-sibling cases.

## Risks

- Risk: overly strict matching drops unusual filenames that relied on bare prefix.
  - Impact: missing history for non-standard names.
  - Mitigation: keep exact legacy match; dated files already use `{key}-{date}` convention (desktop bridge already used the bounded prefix).

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-executor --lib list_transcript_files_rejects_prefix_sibling_session_keys`
  - `cargo test -p skilllite-executor --lib list_plan_files_rejects_prefix_sibling_session_keys`
  - `cargo test -p skilllite-executor --lib`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-executor --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - N/A (unit-covered)

## Regression Scope

- Areas likely affected:
  - Agent chat history loading / `chat_history` tool
  - Plan file listing
  - Desktop recent-plan fallback scan for `default`
- Explicit non-goals:
  - Session-key path traversal (#126)
  - Desktop transcript loader (already used `{key}-` prefix)

## Links

- Source TODO section: critical-bug automation cron
- Related PRs/issues: open critical backlog #89/#123–#134 (excluded)
- Related docs: N/A
