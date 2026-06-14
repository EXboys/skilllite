# Technical Context

## Current State

- Relevant crates/files: to be determined from recent commit diffs.
- Current behavior: current branch `cursor/critical-bug-investigation-d75f` is used for the audit; base branch is `main`.

## Architecture Fit

- Layer boundaries involved: unknown until suspicious changes are selected; repository-wide rules apply.
- Interfaces to preserve: public CLI, sandbox, command, agent, MCP, and Python SDK behavior unless a confirmed bug fix requires a targeted change.

## Dependency and Compatibility

- New dependencies: none planned.
- Backward compatibility notes: no compatibility impact unless a critical fix is implemented.

## Design Decisions

- Decision: perform a review-only audit first and defer implementation until a concrete critical trigger scenario is established.
  - Rationale: the automation explicitly requires a high confidence bar and no PR for doubtful findings.
  - Alternatives considered: proactively patch suspicious code based on pattern matching.
  - Why rejected: it risks false positives and unnecessary behavior drift.

## Open Questions

- [ ] Which recent commits contain high-blast-radius behavioral changes?
- [ ] Does any suspicious change have a concrete trigger scenario with critical impact?
