# PRD

## Background

P7-A and P7-B established goal/capability/environment awareness, but evolution execution still lacks governance. Both proactive and passive triggers can request evolution, yet there is no single arbitration point to avoid collisions, random improvements, or duplicated runs.

## Objective

Deliver a minimum governance layer that converts dual evolution triggers into proposal production and central coordinator arbitration before execution.
Enable safe rollout by defaulting to shadow mode and permitting optional low-risk auto-execution only after explicit enablement.

## Functional Requirements

- FR-1: Define and use a unified `EvolutionProposal` structure with source, scope, risk, effort, expected gain, ROI score, dedupe key, and acceptance criteria.
- FR-2: Add `evolution_backlog` persistence to store proposal lifecycle status.
- FR-3: Add coordinator logic for queue ordering, dedupe, lock, and execution decision.
- FR-4: Active/passive paths must write proposals only; execution must be centralized.
- FR-5: Shadow mode must be default; low-risk auto-exec must be explicit opt-in.

## Non-Functional Requirements

- Security: No elevated privilege or sensitive system probing added; only local SQLite and existing evolution logic are used.
- Performance: Coordinator overhead should be lightweight (single short DB transaction and in-memory ranking).
- Compatibility: Preserve forced/manual evolution behavior and avoid breaking existing command interfaces.

## Constraints

- Technical:
  - Keep crate boundaries unchanged (`skilllite-agent` integration via `skilllite-evolution` APIs).
  - No new external crates.
- Timeline: Fit within P7-C MVP window (2 weeks), with minimal invasive changes.

## Success Metrics

- Metric: Duplicate/competing auto evolution executions caused by dual trigger paths.
- Baseline: Dual trigger paths can both converge to direct execution intent.
- Target: Both paths produce proposals; coordinator chooses single executable candidate.
- Metric: Auto execution safety posture.
- Baseline: Existing behavior can auto-execute once thresholds match.
- Target: Default shadow mode prevents auto execution unless explicitly enabled.

## Rollout

- Rollout plan:
  - Phase 1: Enable shadow mode by default and observe proposal backlog.
  - Phase 2: Enable low-risk auto execution via env toggle after metrics stabilize.
- Rollback plan:
  - Disable low-risk auto execution and/or keep shadow mode on.
  - If needed, fallback to previous behavior by bypassing coordinator path in follow-up patch.
