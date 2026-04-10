# PRD

## Background

The desktop assistant reimplemented skill discovery separately from the core
workspace rules. That duplication already drifted: the assistant only reasons
about `.skills` / `skills` and custom `_evolved` traversal, while core also
supports `.agents/skills` and `.claude/skills` plus legacy fallback behavior.

## Objective

Use core-owned discovery rules as the single source of truth for assistant skill
directory resolution and assistant-visible skill instance enumeration.

## Functional Requirements

- FR-1: Core must expose a reusable discovery helper that returns concrete skill directories suitable for assistant/UI flows, including evolved and pending skill locations derived from canonical skill roots.
- FR-2: Assistant skill list/open/remove and pending-review flows must call core discovery helpers instead of maintaining `.skills` / `skills` / `_evolved` path rules locally.
- FR-3: Assistant workspace root and bundled-skill seeding logic must honor the same canonical skill root resolution.

## Non-Functional Requirements

- Security:
  - No new filesystem access outside the existing workspace skill roots.
- Performance:
  - Discovery remains deterministic and bounded to known skill roots, not broad recursive search across the workspace.
- Compatibility:
  - Existing `.skills` / `skills` behavior must continue to work while adding `.agents/skills` / `.claude/skills` support in assistant flows.

## Constraints

- Technical:
  - Keep dependency direction one-way: assistant consumes core discovery helpers; core must not depend on assistant or evolution.
- Timeline:
  - One implementation pass with focused regression coverage.

## Success Metrics

- Metric: Assistant skill discovery paths are sourced from core helpers instead of local duplicated rules.
- Baseline: Assistant maintains its own `.skills` / `skills` / `_evolved` traversal.
- Target: Assistant uses core helpers for discovery and canonical root resolution.

## Rollout

- Rollout plan: Ship the core helper first, then switch assistant call sites in the same change.
- Rollback plan: Revert the helper adoption and restore the previous assistant-local discovery flow if regressions appear.
