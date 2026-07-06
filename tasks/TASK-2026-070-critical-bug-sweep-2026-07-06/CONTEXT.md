# Technical Context

## Current State

- Relevant crates/files:
  - Initial scope is repository-wide recent commit review.
  - Candidate areas from recent history include evolution workspace scoping, agent/LLM UTF-8 truncation, dependency/security bumps, and sandbox CI coverage.
- Current behavior:
  - Branch `cursor/critical-bug-investigation-4841` is currently aligned with `main` at merge commit `b57e289`.
  - No code change has been made for this sweep yet.

## Architecture Fit

- Layer boundaries involved: To be determined only if a candidate bug is confirmed.
- Interfaces to preserve: CLI behavior, workspace/root resolution semantics, sandbox/security invariants, and task artifact workflow.

## Dependency and Compatibility

- New dependencies: None planned.
- Backward compatibility notes: Any fix must preserve existing shipped behavior except the confirmed regression.

## Design Decisions

- Decision: Start as an investigation-only task and avoid implementation unless the severity/confidence bar is met.
  - Rationale: The automation explicitly expects "no critical bugs found" most days and forbids speculative PRs.
  - Alternatives considered: Proactively patch suspicious code.
  - Why rejected: Suspicion without a concrete trigger does not meet the task's confidence bar.

## Open Questions

- [ ] Which recent behavioral changes have enough blast radius to trace deeply?
- [ ] Does any traced path produce a concrete severe failure scenario?
