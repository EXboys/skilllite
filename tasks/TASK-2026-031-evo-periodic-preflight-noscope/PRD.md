# PRD

## Background

Users saw repeated “no proposal” scheduling outcomes every 10 minutes while A9 periodic arm was due but passive/active proposal gates were closed.

## Objective

Reduce noisy autorun attempts when only the periodic arm fires and no proposals would exist; make empty-proposal audit reasons self-explanatory in logs and UI.

## Functional Requirements

- FR-1: When growth is due solely from the periodic arm and `would_have_evolution_proposals` is false, do not invoke `run_evolution` / `skilllite evolution run` and do not write `evolution_run_outcome` for that tick.
- FR-2: When growth is due from signal or sweep arms, behavior remains attempt autorun even if proposals may still be empty.
- FR-3: When proposals are empty after `build_evolution_proposals`, log a specific reason (disabled, daily cap, cooldown, or passive+active idle).

## Non-Functional Requirements

- Security: No change to sandbox or policy execution paths.
- Performance: Fewer subprocess spawns and LLM-free preflight on skipped ticks.
- Compatibility: `growth_due` return type change is internal to workspace crates; external API stable aside from documented behavior.

## Constraints

- Technical: Desktop preflight must use merged workspace + UI env for thresholds.
- Timeline: Single iteration.

## Success Metrics

- Metric: Volume of `evolution_run_outcome` rows for periodic idle sessions
- Baseline: One per interval when proposals empty
- Target: Zero for periodic-only empty case

## Rollout

- Rollout plan: Ship with next release; docs updated.
- Rollback plan: Revert preflight branches; restore `growth_due` bool if needed.
