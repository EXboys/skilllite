# PRD

## Background

After adding policy runtime and risk budgets, the backlog becomes the primary operations surface for
evolution governance. Users need a first-party CLI command to inspect queued/executed/denied proposals.

## Objective

Provide a direct CLI command to query evolution backlog with lightweight filtering and readable output.

## Functional Requirements

- FR-1: Add `evolution backlog` subcommand in CLI parser and dispatcher.
- FR-2: Return latest backlog records from evolution DB, sorted by `updated_at DESC`.
- FR-3: Support optional `--status`, `--risk`, `--limit` filters.

## Non-Functional Requirements

- Security:
  - Command is read-only and must not mutate backlog state.
- Performance:
  - Query should use existing indexes and default to bounded row count.
- Compatibility:
  - Existing `evolution` subcommands remain backward compatible.

## Constraints

- Technical:
  - Reuse existing `feedback::open_evolution_db` and no new dependency.
- Timeline:
  - Same-day incremental feature.

## Success Metrics

- Metric: Backlog inspection requires no manual SQLite tooling.
- Baseline: Operators must query DB manually.
- Target: One CLI command gives filtered backlog view.

## Rollout

- Rollout plan:
  - Ship command with default limit and table output.
- Rollback plan:
  - Remove dispatch binding and CLI variant if regression occurs.
