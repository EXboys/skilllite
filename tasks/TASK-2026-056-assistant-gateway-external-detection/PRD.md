# PRD

## Background

The Assistant now supports one-click managed startup for `skilllite gateway serve`, but real local setups may already have a terminal- or service-managed gateway on the same bind. In that case, the desktop UI should recognize the healthy external listener instead of only reporting `AddrInUse`.

## Objective

The settings page reports a healthy external gateway as a running state on the configured bind, and the start action downgrades from a confusing bind-collision failure to an informative external-running result.

## Functional Requirements

- FR-1:
- FR-2:

## Non-Functional Requirements

- Security:
- Performance:
- Compatibility:

## Constraints

- Technical:
- Timeline:

## Success Metrics

- Metric:
- Baseline:
- Target:

## Rollout

- Rollout plan:
- Rollback plan:
