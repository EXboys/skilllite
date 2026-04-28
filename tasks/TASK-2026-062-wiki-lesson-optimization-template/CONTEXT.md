# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/types/feedback.rs`
  - `crates/skilllite-commands/src/wiki.rs`
  - EN/ZH README and architecture docs.
- Current behavior: `wiki_update_suggestion` and `wiki record-lesson` exist, but the default lesson text is a compact summary.

## Architecture Fit

- Layer boundaries involved: agent builds prompt payload; commands write Markdown.
- Interfaces to preserve: confirmed-only writes, Markdown-only Repo Wiki, no memory/SQLite changes.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Existing `record-lesson --body` remains supported.

## Design Decisions

- Decision: Use deterministic section headings rather than LLM summarization.
  - Rationale: Safe, testable, and avoids adding model calls after a run completes.
  - Alternatives considered: Ask LLM to summarize the lesson.
  - Why rejected: Larger surface area and harder verification for this small refinement.

## Open Questions

- [ ] Future UI may allow editing the proposed lesson before confirmation.
