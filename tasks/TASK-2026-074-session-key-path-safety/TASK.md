# TASK Card

## Metadata

- Task ID: `TASK-2026-074`
- Title: Reject path-escaping chat session keys
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-28`
- Target milestone:

## Problem

Chat transcript and plan paths join attacker-controlled `session_key` with `Path::join`. Absolute keys (e.g. `/tmp/evil`) replace the transcripts/plans directory root, and `../` keys escape it. Concrete triggers include CLI `chat` / `clear-session`, `agent-rpc`, schedule jobs, and executor transcript/plan RPC.

## Scope

- In scope:
  - Validate session keys as a single safe path segment.
  - Harden transcript/plan path builders and key write/read entry points.
  - Regression tests for absolute/traversal keys.
- Out of scope:
  - Broader chat workspace split-brain fixes already covered by open PRs.
  - Skill-name / artifact-key escapes already covered by open PRs #89/#123/#125.

## Acceptance Criteria

- [x] Absolute and traversal `session_key` values are rejected before filesystem join.
- [x] Valid keys such as `default`, `s-...`, `schedule-*` still resolve under transcripts/plans.
- [x] Regression tests cover absolute-path write attempts and path builders.
- [x] Affected crates format/clippy/tests pass for the change scope.

## Risks

- Risk: Existing unusual session keys containing `/` become invalid.
  - Impact: Those sessions stop writing/reading until renamed.
  - Mitigation: Reject only unsafe path forms; keep hyphen/underscore/unicode single-segment keys.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core path_validation`
  - `cargo test -p skilllite-executor --lib`
  - `cargo test -p skilllite-agent --lib`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-core -p skilllite-executor -p skilllite-agent --all-targets -- -D warnings`
- Manual checks:
  - Confirm `/tmp/...` session key cannot create files outside transcripts dir.

## Regression Scope

- Areas likely affected:
  - Chat transcript append/read, plan append/read, schedule tick session keys, agent-rpc, clear-session.
- Explicit non-goals:
  - Desktop Life Pulse rhythm `--workspace` (open PR #116).
  - Skill directory name escapes (open PR #125).

## Links

- Source TODO section: critical-bug-investigation automation
- Related PRs/issues: same class as #123 / #125
- Related docs: N/A (validation rejects unsafe keys; CLI default keys unchanged)
