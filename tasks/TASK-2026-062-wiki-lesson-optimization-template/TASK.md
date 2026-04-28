# TASK Card

## Metadata

- Task ID: `TASK-2026-062`
- Title: Wiki lesson optimization template
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

Conversation Wiki suggestions currently produce a short lesson summary. User wants confirmed Wiki writes to capture both experience and optimization guidance, not raw transcript or vague notes.

## Scope

- In scope: Structure suggested/recorded lessons with experience and optimization sections (`What Happened`, `Root Cause`, `Optimization`, `Next Time`).
- Out of scope: LLM summarization, full transcript storage, background writes, memory/SQLite changes, Desktop UI rendering.

## Acceptance Criteria

- [x] `wiki_update_suggestion.proposed_lesson` uses a structured experience/optimization template.
- [x] `wiki record-lesson` writes structured sections even when the caller only provides a summary.
- [x] Tests assert recorded lessons contain the required sections.
- [x] Docs clarify that Wiki writes store lessons and optimization guidance, not transcripts.

## Risks

- Risk: Template may be generic when runtime facts are sparse.
  - Impact: Some lessons may need user editing.
  - Mitigation: Keep Markdown human-editable and include concrete runtime facts when available.

## Validation Plan

- Required tests: Unit tests for suggestion template and record-lesson output.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p skilllite-agent`, `cargo test -p skilllite-commands`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: CLI help/docs describe lesson behavior only.

## Regression Scope

- Areas likely affected: Agent suggestion payload, wiki record-lesson output, docs.
- Explicit non-goals: UI rendering, memory, SQLite, watcher, automatic writes.

## Links

- Source TODO section: User clarified Wiki writes should store experience and optimization experience.
- Related PRs/issues:
- Related docs: `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
