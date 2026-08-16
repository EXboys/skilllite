# PRD

## Background

`write_file` and `write_output` skip strict JSON schema validation so a truncated LLM tool-call can still be recovered. Recovery is documented as a way to save partial content when `SKILLLITE_MAX_TOKENS` cuts a large write. That path currently treats a content string that never received any characters as `""` and then overwrites the target.

## Objective

- Existing files are not wiped when recovered write arguments have empty or missing `content`.
- Partial non-empty recovery and intentional empty writes keep working.

## Functional Requirements

- FR-1: If `serde_json::from_str` fails and recovery yields missing or empty `content`, do not call `write_file` / `write_output`; return an error result.
- FR-2: If recovery yields non-empty `content`, keep writing and keep the existing truncation warning.
- FR-3: If the arguments are valid JSON with `content: ""`, keep the current overwrite-with-empty behavior.

## Non-Functional Requirements

- Security: fail closed on empty recovered overwrites; do not broaden write permissions.
- Performance: no new I/O or allocations on the happy path beyond one string check.
- Compatibility: valid JSON callers are unchanged.

## Constraints

- Technical: keep the recovery helper's parse behavior; gate at `execute_builtin_tool` so both write tools share one check.
- Timeline: N/A (automation sweep).

## Success Metrics

- Metric: existing-file wipe on empty recovered JSON
- Baseline: file becomes 0 bytes and tool returns success
- Target: file unchanged and tool returns `is_error`

## Rollout

- Rollout plan: merge the focused agent fix.
- Rollback plan: revert the recovery gate commit.
