# PRD

## Background

Current default skill-directory compatibility logic is duplicated across multiple entry points, creating high maintenance cost and consistency risk.
As IDE/MCP/CLI entry points grow, lack of centralized logic increases the chance of hidden regressions.

## Objective

Provide a unified directory resolution helper so default `skills -> .skills` compatibility behaves consistently across all entry points.
Improve observability by emitting explicit warnings when duplicate skill names exist in both `skills` and `.skills`.

## Functional Requirements

- FR-1: Provide a reusable unified helper that supports default fallback detection (`skills` or `./skills`).
- FR-2: Detect duplicate-name conflicts between `skills/<name>/SKILL.md` and `.skills/<name>/SKILL.md`, and return warning data.
- FR-3: `init`, `skill common`, `ide`, and `mcp` must use the same helper and remove duplicated implementations.

## Non-Functional Requirements

- Security:
  - Do not relax path boundaries or introduce additional filesystem writes.
- Performance:
  - Run conflict detection only in default-directory mode; keep scan scope limited to two levels.
- Compatibility:
  - Preserve existing default fallback behavior to avoid breaking legacy `.skills` users.

## Constraints

- Technical:
  - Respect crate layering boundaries and avoid reverse dependencies.
- Timeline:
  - Complete helper extraction, call-site migration, and minimal regression coverage in one change set.

## Success Metrics

- Metric:
  - Remove duplicated fallback logic at entry points and unify helper usage.
- Baseline:
  - At least four duplicated implementations exist; duplicate conflicts currently have no visible warning.
- Target:
  - All four entry points unified; duplicate coexistence warns correctly; required validation suite passes.

## Rollout

- Rollout plan:
  - Ship directly on main branch as an internal consistency improvement.
- Rollback plan:
  - If path semantics regress, roll back and restore previous per-entry implementation incrementally.
