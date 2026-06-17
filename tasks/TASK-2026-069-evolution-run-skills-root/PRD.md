# PRD

## Background

The recent desktop L2 evolution bridge delegates pending/status/confirm operations to CLI JSON commands. Those read pending evolved skills from the effective workspace skills directory, preferring `skills/` and falling back to `.skills/` only for legacy workspaces. The evolution run command still writes generated pending skills under `.skills/`, creating a split-brain path when `skills/` exists.

## Objective

Ensure evolution-generated pending skills are written to the same effective skills root that desktop pending/status/confirm operations read.

## Functional Requirements

- FR-1: `skilllite evolution run --workspace <ws>` must resolve its skills root with the shared `skills/` plus legacy fallback policy.
- FR-2: `.skills`-only workspaces must remain compatible.
- FR-3: Regression tests must prove both root-selection cases.

## Non-Functional Requirements

- Security: No new path traversal or permission bypass behavior.
- Performance: No measurable impact; root resolution is local path checking only.
- Compatibility: Preserve `.skills` fallback for existing legacy projects.

## Constraints

- Technical: Keep the fix in command-layer Rust code and reuse existing lower-layer discovery helpers.
- Timeline: N/A for autonomous execution.

## Success Metrics

- Metric: Root selected by `evolution run` matches desktop pending/status root.
- Baseline: Workspaces with `skills/` present write pending skills under `.skills`.
- Target: Workspaces with `skills/` present write pending skills under `skills`, while `.skills`-only workspaces still use `.skills`.

## Rollout

- Rollout plan: Ship as a narrow bug fix with command-level regression tests.
- Rollback plan: Revert the command helper change and tests if unexpected compatibility issues appear.
