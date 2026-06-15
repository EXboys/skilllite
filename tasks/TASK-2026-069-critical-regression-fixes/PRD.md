# PRD

## Background

The daily critical-bug automation reviewed recent UTF-8 truncation and evolution
workspace DB fixes. The review found two crash-class UTF-8 preview paths that still
slice strings by byte index, desktop evolution subprocesses that do not carry the
workspace selected by the UI, and prompt injection paths that bypass the existing
high-risk `SKILL.md` security notice for some documentation sources.

## Objective

Prevent concrete crash and workspace-split scenarios with minimal changes, and
reuse the existing skill documentation security notice for all injected high-risk
skill docs.

## Functional Requirements

- FR-1: Error preview truncation for embedding responses and task-planner parse
  failures must be UTF-8 safe.
- FR-2: Desktop life-pulse growth and authorized capability background runs must
  invoke `skilllite evolution run` with `--workspace <selected workspace>`.
- FR-3: High-risk skill reference docs and bash-tool docs injected into prompts
  must include the existing security notice.
- FR-4: Tests must exercise non-ASCII boundary cases and prompt notice injection.

## Non-Functional Requirements

- Security: Do not relax sandbox, command, network, or approval policies.
- Performance: Keep checks local string scans only; no additional I/O beyond already
  loaded documentation files.
- Compatibility: Preserve existing public CLI flags, env vars, response formats,
  and skill metadata structures.

## Constraints

- Technical: Avoid broad refactors; use existing helpers such as `safe_truncate`
  and `has_skill_md_high_risk_patterns`.
- Timeline: N/A for autonomous execution; scope is bounded to the identified
  critical bug paths.

## Success Metrics

- Metric: Non-ASCII preview paths panic-free.
- Baseline: Byte slicing at fixed offsets can panic when a multibyte character
  crosses the boundary.
- Target: Regression tests pass and code uses Unicode-safe truncation.

## Rollout

- Rollout plan: Ship as a small bug-fix PR on the automation branch.
- Rollback plan: Revert the single fix commit if regressions appear; no migrations
  or persisted schema changes are involved.
