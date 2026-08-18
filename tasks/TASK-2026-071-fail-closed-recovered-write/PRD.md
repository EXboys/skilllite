# PRD

## Background

`write_file` / `write_output` accept invalid or truncated tool-call JSON and recover `path` / `content` / `append` with regex. That recovery is required because LLM streams often hit `SKILLLITE_MAX_TOKENS` mid-argument. On main it is too eager: it can write to the wrong path and it can overwrite an existing file when `append: true` was truncated away.

## Objective

Recovered writes must not destroy existing file contents or write to a path found inside `content`. New-file partial recovery and valid JSON behavior stay available.

## Functional Requirements

- FR-1: Recovered `path` / `file_path` may only come from JSON keys that appear before the first `"content"` key.
- FR-2: A recovered write without `append: true` that targets an existing non-empty file must fail and leave the file unchanged.
- FR-3: A recovered write to a missing path may still create the file with recovered content and the truncation warning.
- FR-4: A recovered write with `"append":true` before `content` must still append.
- FR-5: Valid JSON `write_file` / `write_output` overwrite and append are unchanged.

## Non-Functional Requirements

- Security: Do not write recovered content to an unintended workspace path.
- Performance: Keep recovery as a regex prefix scan; no new dependencies.
- Compatibility: Do not change valid JSON tool arguments.

## Constraints

- Technical: Minimal change in `helpers.rs` and `execute_builtin_tool`. No overlap with PR #144's empty-content predicate.
- Timeline: Same automation run.

## Success Metrics

- Metric: Regression tests for append-loss and inner-path recovery.
- Baseline: Both cases succeed and mutate the wrong/existing file on main.
- Target: Both cases return `is_error` and leave victim files intact.

## Rollout

- Rollout plan: Merge after tests/clippy/fmt pass.
- Rollback plan: Revert the recovery prefix + clobber-check commit.
