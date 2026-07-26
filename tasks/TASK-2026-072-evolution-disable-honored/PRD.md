# PRD

## Summary

`evolution disable` must actually stop a mutable planning rule from influencing future plans, while keeping the rule record on disk so users can re-enable it by editing `rules.json`.

## User-visible behavior

- Before: `skilllite evolution disable <id>` prints success, but the rule still appears in planning prompts and beliefs.
- After: the same command still writes `"disabled": true`, and subsequent plan/beliefs loading ignores that rule.

## Requirements

1. The runtime rule model must retain the `disabled` flag across read/write cycles.
2. Planner and beliefs consumers must treat `disabled: true` as inactive.
3. Evolution merge/retire helpers must not silently drop disabled rules from `rules.json` just because they are inactive.

## Non-goals

- Workspace argument plumbing for disable/explain.
- Automatic re-enable command.
- Deleting disabled rules from disk.
