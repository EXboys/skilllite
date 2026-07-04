# PRD

## Background

Recent evolution workspace fixes aligned several CLI and desktop DB paths, but desktop chat and
desktop evolution UI still disagree when the configured workspace is a nested directory under a
parent project that contains `skills/`, `.skills/`, or another supported skill root. Chat uses the
canonical project root; evolution UI subprocesses can use the raw nested path.

## Objective

All desktop evolution subprocesses should target the same workspace root as desktop chat/A9 for
database and skill-root operations, while preserving existing `.env` lookup behavior.

## Functional Requirements

- FR-1: Build evolution CLI `--workspace` arguments from `find_project_root(raw_workspace)`.
- FR-2: Continue passing the raw workspace to subprocess helpers for dotenv discovery and current
  directory setup.
- FR-3: Cover nested workspace canonicalization in tests for the commands that build arguments
  locally in the desktop bridge.

## Non-Functional Requirements

- Security: Do not broaden filesystem access or bypass existing workspace validation.
- Performance: Canonicalization should reuse the existing bounded parent walk and not add long
  scans.
- Compatibility: Preserve the established chat/A9 interpretation of a project root as the nearest
  parent with a supported skill root.

## Constraints

- Technical: Keep the change in the desktop bridge layer; do not add new crate dependencies or alter
  lower-layer CLI parsing.
- Timeline: No calendar estimate; the fix is a small boundary alignment with focused regression
  tests.

## Success Metrics

- Metric: Nested workspace command arguments point at the parent project root.
- Baseline: Raw nested workspace is passed to `--workspace`.
- Target: Canonical project root is passed to `--workspace` for desktop evolution operations.

## Rollout

- Rollout plan: Ship as a desktop bridge bug fix with regression tests.
- Rollback plan: Revert the desktop bridge helper and call-site changes if unexpected workspace
  selection regressions appear.
