# PRD

## Background

Current planning has improved goal awareness, but capability awareness is still implicit. The agent cannot explicitly present capability coverage or quantify gaps against user goals.

## Objective

Provide an explicit capability map and capability-gap report during planning so task decomposition can be constrained by what the agent can do today and what it still lacks.

## Functional Requirements

- FR-1: Build a capability registry from callable skills and tool metadata.
- FR-2: Build a capability gap analyzer that infers required capability domains from user goal inputs.
- FR-3: Inject compact capability registry and gap report blocks into planning input.
- FR-4: Report required/covered/missing domains and a quantified gap ratio.

## Non-Functional Requirements

- Security: No new tool permissions; analysis is read-only and prompt-level.
- Performance: Keep analysis deterministic and lightweight (no extra mandatory external IO).
- Compatibility: Preserve existing planning flow and fallback behavior.

## Constraints

- Technical: Changes remain inside `skilllite-agent` crate with no dependency direction change.
- Timeline: Deliver MVP with deterministic domain inference.

## Success Metrics

- Metric: Planning prompt includes capability-awareness context.
- Baseline: Planner sees skills list but no explicit capability map/gap quantification.
- Target: Required/covered/missing domains and ratio are present for goals with inferable domain requirements.

## Rollout

- Rollout plan: Enable by default in planning input assembly with compact text blocks.
- Rollback plan: Remove module wiring from task planner and planning phase.
