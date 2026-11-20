# PRD

## Background

Sandbox and bash execution share `wait_with_timeout`. After a child exits, the function reads `ru_maxrss` for all waited-for children and, when that value is above the configured limit, drops stdout and reports a memory failure. `ru_maxrss` never goes down. A long-lived `skilllite chat` or agent process, or one `skilllite run` that already waited for a large helper, then treats later small commands as over the limit.

## Objective

A finished command is rejected for memory only when that command raises the process-wide child RSS high-water mark above its own limit. Successful later commands keep their stdout.

## Functional Requirements

- FR-1: If the child RSS high-water mark does not increase while this child is reaped, return that child's stdout, stderr, and exit code.
- FR-2: If the mark increases and the new mark is above this child's limit, drop stdout and return the existing peak-memory rejection.
- FR-3: The live RSS poll is unchanged and still kills a child that stays above the limit.

## Non-Functional Requirements

- Security: filesystem, network, and sandbox level rules stay the same. The memory check remains fail-closed for a child that sets a new peak above its limit.
- Performance: one extra `getrusage` call per poll interval.
- Compatibility: `SKILLLITE_MAX_MEMORY_MB` keeps its default and meaning. No new environment variable.

## Constraints

- Technical: `RUSAGE_CHILDREN` cannot report this child's peak once a higher historical peak exists. Do not switch to the direct child's `VmHWM`, because a bwrap parent would hide the sandboxed grandchild.
- Timeline: minimal fix on the current main branch.

## Success Metrics

- Metric: stdout of a small command after a larger child.
- Baseline: stdout is empty and the result is `memory_limit`.
- Target: stdout is preserved and the process is not marked killed.

## Rollout

- Rollout plan: ship with the sandbox crate. No migration.
- Rollback plan: revert the comparison and the regression tests.
