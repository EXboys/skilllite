# PRD

## Background

The desktop chat UI authorizes one backlog proposal and polls that proposal for completion. The
background runner currently loses this identity at the CLI boundary, leaving the selected proposal
queued and allowing forced execution to select unrelated work.

## Objective

The proposal ID returned by `authorize-capability` must reach `evolution run` as the explicit
`--proposal-id` CLI argument.

## Functional Requirements

- FR-1: The authorization subprocess argument vector includes the selected proposal ID.
- FR-2: Workspace routing and JSON output behavior remain unchanged.

## Non-Functional Requirements

- Security: User authorization must not be applied to a different proposal.
- Performance: No additional process, I/O, or network work.
- Compatibility: Use the existing `evolution run --proposal-id` interface.

## Constraints

- Technical: Keep the change local to the desktop authorization bridge.
- Timeline: N/A for autonomous execution.

## Success Metrics

- Metric: Authorized proposal identity preserved across the process boundary.
- Baseline: Zero; the ID is removed by `cmd_run` when the explicit argument is absent.
- Target: Every authorization-triggered run carries the selected non-empty proposal ID.

## Rollout

- Rollout plan: Ship as a minimal desktop bridge bug fix.
- Rollback plan: Revert the argument-construction change and its regression test.
