# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-agent/src/agent_loop/helpers.rs`, `crates/skilllite-agent/src/task_planner.rs`, and recent UTF-8 truncation commits on `main`.
- Current behavior: Agent planning error paths now use `safe_truncate` for previews instead of raw byte slicing.

## Architecture Fit

- Layer boundaries involved: `skilllite-agent` planning and agent-loop error handling.
- Interfaces to preserve: Tool-result error reporting, planner parse/fallback behavior, and existing `safe_truncate` utility semantics.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: No API, CLI, schema, or persisted-format changes; invalid non-ASCII planner inputs now return errors instead of panicking.

## Design Decisions

- Decision: Fix only the confirmed agent crash paths in this PR.
  - Rationale: `update_task_plan` and `parse_task_list` are active agent/planning paths where LLM-produced non-ASCII invalid data can panic before returning a recoverable error.
  - Alternatives considered: Sweep every remaining byte slice across CLI/admin formatting paths.
  - Why rejected: Several remaining paths are lower-blast-radius or ASCII/index-derived; broad cleanup would exceed the critical-bug scope.

## Open Questions

- [x] Which recent commits have the largest behavioral blast radius?
- [x] Do any suspicious changes have a concrete critical trigger scenario?
