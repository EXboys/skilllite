# PRD

## Background

The desktop assistant shells out to a `skilllite` subprocess. In debug builds, the
current path resolution prefers `~/.skilllite/bin/skilllite`, which can lag behind
the checked-out workspace source. That makes desktop debugging unreliable because
the GUI may execute stale CLI logic even after local code changes compile.

## Objective

Ensure debug desktop sessions prefer the workspace-built `skilllite` binary so the
GUI executes the same code that developers just built from the current repository.

## Functional Requirements

- FR-1: Debug builds must probe the workspace `target/debug/skilllite` before
  checking `~/.skilllite/bin/skilllite`.
- FR-2: If the workspace debug binary is absent, resolution must continue to the
  existing bundled / home-bin / PATH fallback chain.

## Non-Functional Requirements

- Security: No broader file access than local path existence checks.
- Performance: Path probing should stay constant-time and negligible.
- Compatibility: Release behavior must remain unchanged.

## Constraints

- Technical: Keep the change inside desktop path resolution; do not modify CLI protocol.
- Timeline: Small unblocker fix for local desktop development.

## Success Metrics

- Metric: Desktop debug runs execute current-workspace CLI behavior.
- Baseline: A stale `~/.skilllite/bin/skilllite` can shadow the just-built workspace binary.
- Target: Workspace debug binary wins when present.

## Rollout

- Rollout plan: Ship as a debug-resolution improvement with no release-facing behavior change.
- Rollback plan: Revert the new workspace candidate preference and keep old fallback order.
