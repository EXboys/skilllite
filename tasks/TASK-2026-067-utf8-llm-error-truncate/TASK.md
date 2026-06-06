# TASK Card

## Metadata

- Task ID: `TASK-2026-067`
- Title: UTF-8 safe LLM error truncation
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-06`
- Target milestone:

## Problem

LLM API error formatting truncates non-JSON response bodies with byte slicing. A proxy or provider error page containing CJK or emoji can panic when the 200-byte boundary lands inside a UTF-8 code point, crashing the agent/chat error path instead of returning a user-readable API error.

## Scope

- In scope: make LLM API error fallback truncation UTF-8 safe; harden the same byte-slice truncation pattern in skill reference prompt assembly; add non-ASCII regression coverage.
- Out of scope: broad LLM error-handling redesign, provider-specific parsing changes, UI changes, or docs changes for unchanged user-facing semantics.

## Acceptance Criteria

- [ ] `format_api_error` handles long non-JSON CJK/emoji bodies without panic and still includes the friendly status hint plus an ellipsis.
- [ ] Skill reference content truncation does not split UTF-8 code points.
- [ ] Relevant Rust tests, formatting, clippy, and task validation pass.

## Risks

- Risk: changing truncation length semantics.
  - Impact: error summaries or prompt excerpts could include slightly fewer bytes when the limit falls inside a multibyte character.
  - Mitigation: reuse the existing `safe_truncate` helper and preserve existing byte ceilings.

## Validation Plan

- Required tests: focused regression tests in `skilllite-agent`.
- Commands to run: `cargo test -p skilllite-agent`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: inspect modified files and task artifacts after edits.

## Regression Scope

- Areas likely affected: LLM API HTTP error formatting and prompt assembly for skill reference files.
- Explicit non-goals: changing provider retry behavior, adding new dependencies, or modifying assistant UI behavior.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
