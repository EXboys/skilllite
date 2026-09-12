# PRD

## Problem Statement

Agent and tool history loaders can mix another session's transcripts/plans when the requested session key is a string prefix of a sibling key. That leaks private chat context into the LLM and returns incorrect user-facing history.

## Goals

- Enforce a filename boundary after `session_key` for dated transcript/plan files.
- Preserve legacy exact-name transcript/plan files.
- Lock behavior with falsifiable regression tests.

## Non-Goals

- Redesigning session key generation.
- Fixing unrelated evolution backlog uniqueness semantics.
- Broad session-store concurrency redesign.

## User Stories

- As a user with sessions `s1` and `s10`, when I continue `s1`, the agent must not see `s10` messages.
- As a scheduler using `schedule-1` and `schedule-10`, plan/history lookups for one job must not include the other.

## Requirements

- Functional:
  - Dated transcript match: `{session_key}-*.jsonl` only, plus legacy `{session_key}.jsonl`.
  - Plan listing: stem equals `session_key` or starts with `{session_key}-`.
- Non-functional:
  - Minimal diff; no schema/API changes.

## Success Metrics

- Regression tests fail if bare `starts_with(session_key)` is restored.
- Existing same-session dated + legacy files still resolve.
