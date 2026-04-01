# PRD

## Background

Capability and gap awareness are available, but local runtime readiness is still implicit. Missing local tools can cause avoidable execution failures after planning.

## Objective

Provide a safe, lightweight environment profile for planning that reports whether key developer tools exist and their versions.

## Functional Requirements

- FR-1: Check availability of `git`, `python`, `node`, `npm`, and `cargo`.
- FR-2: Capture concise version info via read-only commands (`--version`).
- FR-3: Inject an environment profile block into planning user content.
- FR-4: Keep behavior deterministic and non-privileged.

## Non-Functional Requirements

- Security: No privileged commands; no sensitive path reads; fixed tool allowlist.
- Performance: Keep checks short and local process-level only.
- Compatibility: Additive prompt context only; no protocol breaking changes.

## Constraints

- Technical: Implement inside `skilllite-agent` as a small utility module.
- Timeline: MVP delivery in this task.

## Success Metrics

- Metric: Planner receives explicit local tool readiness information.
- Baseline: No explicit local tool readiness block in planning context.
- Target: Environment profile block present with per-tool availability and version summary.

## Rollout

- Rollout plan: Enable by default in planning content assembly.
- Rollback plan: Remove env profiler block generation and injection calls.
