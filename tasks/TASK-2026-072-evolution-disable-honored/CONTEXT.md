# Context

## Technical boundaries

- `PlanningRule` lives in `crates/skilllite-core/src/planning.rs` and is shared by agent + evolution.
- `cmd_disable` already writes `"disabled": true` via raw `serde_json::Value` in `crates/skilllite-commands/src/evolution.rs`.
- Consumption paths:
  - `skilllite_agent::planning_rules::load_rules` → `TaskPlanner`
  - `skilllite_agent::soul::build_beliefs_block` via `seed::load_rules`
- Storage/merge paths (`seed::load_rules`, prompt/external learners) must keep disabled rules so write-backs do not erase them.

## Constraints

- Do not filter inside `seed::load_rules`; that helper feeds merge/write paths.
- Prefer a small schema + filter change over redesigning disable UX.
- Preserve default serde behavior for existing rules that omit `disabled` (treat as active).

## Compatibility notes

- Existing `rules.json` files without `disabled` remain valid.
- Serializing active rules may omit `"disabled": false` via `skip_serializing_if`.
