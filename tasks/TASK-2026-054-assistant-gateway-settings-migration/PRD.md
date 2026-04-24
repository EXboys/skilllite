# PRD

## Background

The Assistant has already switched product copy and code-facing naming to `gateway serve`, but persisted local settings still carry legacy `channelServe*` compatibility fields. This keeps the runtime shape more complex than necessary and postpones the actual migration of old data.

## Objective

Existing local settings are migrated forward automatically at load time, and runtime Assistant code no longer depends on legacy `channelServe*` fallback reads.

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
