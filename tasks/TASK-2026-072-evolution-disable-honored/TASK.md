# TASK Card

## Metadata

- Task ID: `TASK-2026-072`
- Title: Honor evolution disable for planning rules
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-26`
- Target milestone:

## Problem

`skilllite evolution disable <rule_id>` writes `"disabled": true` into `prompts/rules.json` and reports success, but the planner deserializes rules into `PlanningRule` (which has no `disabled` field) and never filters disabled rules. A harmful evolved planning rule therefore continues to affect every future plan after the user believes it was disabled.

## Scope

- In scope:
  - Persist `disabled` on `PlanningRule`
  - Exclude disabled rules from planner/beliefs consumption paths
  - Regression tests for deserialize + filter behavior
- Out of scope:
  - Adding `--workspace` to `disable`/`explain` (separate workspace-scoping follow-up)
  - Changing the disable CLI UX or removing rules from disk

## Acceptance Criteria

- [ ] `PlanningRule` round-trips a `disabled` JSON field without dropping it
- [ ] Disabled rules are not injected into planning prompts / beliefs
- [ ] Active (non-disabled) rules continue to load and match as before
- [ ] Unit/regression tests cover the disable-honored path

## Risks

- Risk: Filtering inside `seed::load_rules` could drop disabled rules during evolution merge write-backs
  - Impact: Permanent loss of disabled rule records
  - Mitigation: Keep disabled rules in storage/load helpers; filter only at planner/beliefs consumption

## Validation Plan

- Required tests:
  - Serde round-trip for `disabled`
  - `planning_rules::load_rules` excludes disabled entries
  - Rule filter used by the planner skips disabled entries
- Commands to run:
  - `cargo test -p skilllite-core planning`
  - `cargo test -p skilllite-agent planning_rules filter_rules`
  - `cargo fmt --check`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - N/A (unit coverage sufficient for the consumption path)

## Regression Scope

- Areas likely affected:
  - Planning rule deserialize/serialize
  - Task planner rule injection
  - Beliefs block derived from evolved rules
- Explicit non-goals:
  - Workspace scoping for disable/explain CLI
  - Skill-add / MCP path traversal issues found in the same sweep

## Links

- Source TODO section: critical bug automation cron
- Related PRs/issues: open follow-ups #110/#113/#120 (reset), #89 (pending path)
- Related docs: N/A
