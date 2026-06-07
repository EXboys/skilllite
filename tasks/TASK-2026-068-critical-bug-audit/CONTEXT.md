# Technical Context

## Current State

- Relevant crates/files: To be determined from recent commit review; likely `crates/`, `docs/`, `python-sdk/`, and task artifacts touched by recent commits.
- Current behavior: Current branch `cursor/critical-bug-investigation-b158` is aligned with `origin/main` at audit start.

## Architecture Fit

- Layer boundaries involved: Potentially all Rust workspace layers depending on reviewed commits.
- Interfaces to preserve: Public CLI behavior, sandbox execution policy, agent/tool orchestration contracts, persisted data formats.

## Dependency and Compatibility

- New dependencies: None expected for audit-only work.
- Backward compatibility notes: No compatibility changes unless a confirmed critical fix requires it.

## Design Decisions

- Decision: Use evidence-based triage before any fix.
  - Rationale: The automation should not open PRs for speculative or low-impact observations.
  - Alternatives considered: Broad refactoring or proactive hardening without a confirmed trigger.
  - Why rejected: It would increase review noise and risk outside the requested scope.

## Open Questions

- [ ] Which recent commits have the largest behavioral blast radius?
- [ ] Do any suspicious changes have a concrete critical trigger scenario?
