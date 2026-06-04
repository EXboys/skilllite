# PRD

## Background

Recent desktop split work moved authorized capability evolution to a CLI subprocess. The CLI supports forcing a backlog proposal with `--proposal-id`, and `cmd_run` intentionally scopes the force environment variable around that explicit argument. The desktop authorization path did not pass the argument, so the authorized proposal id could be lost before `run_evolution` selected work.

## Objective

When a user starts evolution from the chat capability prompt, the immediately spawned background run must target the exact proposal id returned by `authorize-capability`.

## Functional Requirements

- FR-1: The authorization background process must include `--proposal-id` followed by the authorized proposal id.
- FR-2: Existing `.env` and child environment merging must continue to work.
- FR-3: Manual evolution triggering remains unchanged.

## Non-Functional Requirements

- Security: Do not bypass coordinator policy runtime or auto-approve dangerous evolution; only preserve target selection.
- Performance: No additional process or database work beyond the existing background run.
- Compatibility: Preserve the existing `skilllite evolution run --proposal-id` CLI contract and current desktop API shape.

## Constraints

- Technical: Keep the fix local to the desktop bridge unless runtime evidence shows the CLI contract itself is broken.
- Timeline: N/A for automation execution; complete within this investigation branch.

## Success Metrics

- Metric: authorization background run argument list.
- Baseline: `skilllite evolution run --json` with only `SKILLLITE_EVO_FORCE_PROPOSAL_ID`.
- Target: `skilllite evolution run --json --proposal-id <id>`, with optional environment also harmless.

## Rollout

- Rollout plan: ship as a focused bug-fix PR.
- Rollback plan: revert the assistant bridge argument helper if unexpected subprocess behavior appears.
