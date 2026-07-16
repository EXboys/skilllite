# Technical Context

## Current State

- Relevant crates/files:
  - Initial scope is repository-wide recent commit review.
  - The only commit added to `origin/main` since 2026-07-14 is merge commit `74f8417` for PR #111.
  - PR #111 changes desktop evolution authorization arguments plus task evidence.
- Current behavior:
  - Branch `cursor/critical-bug-investigation-bfa0` starts aligned with `origin/main` at `74f8417`.
  - The previous fix passes an authorized evolution proposal ID through the typed `--proposal-id` CLI argument.

## Architecture Fit

- Layer boundaries involved: Desktop assistant bridge invoking the CLI command layer.
- Interfaces to preserve: CLI argument behavior, proposal authorization identity, workspace scoping, and security invariants.

## Dependency and Compatibility

- New dependencies: None planned.
- Backward compatibility notes: Investigation does not authorize behavior changes; any fix must preserve existing CLI contracts.

## Design Decisions

- Decision: Start as investigation-only and avoid implementation unless the severity and confidence bars are met.
  - Rationale: The automation explicitly expects no finding most days and forbids speculative fix PRs.
  - Alternatives considered: Proactively patch suspicious code.
  - Why rejected: Suspicion without a concrete trigger does not meet the requested confidence bar.

## Open Questions

- [ ] Does the merged authorization fix introduce any severe regression in argument construction or command parsing?
- [ ] Are there other unreviewed recent behavioral commits with meaningful blast radius?
