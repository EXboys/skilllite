# PRD

## Background

Memory flush exists to let the model persist durable notes before compaction.
It currently reuses the full agent loop and tool surface under a silent sink.
That turns a background housekeeping turn into an unattended mutating agent.

## Objective

- Memory flush may only call memory tools (`memory_search`, `memory_write`, `memory_list`).
- Workspace mutation tools, process execution, skills, and MCP must be unavailable during flush.

## Functional Requirements

- FR-1: Flush turn registry excludes builtin mutating/exec tools (`write_file`, `run_command`, etc.).
- FR-2: Flush turn registry excludes skills and MCP tools.
- FR-3: Flush turn registry still exposes memory tools when memory is enabled.
- FR-4: Normal chat/RPC agent turns are unchanged (full tool access).

## Non-Functional Requirements

- Security: Silent background turns must not be able to rewrite the workspace.
- Compatibility: No new env vars or CLI flags.
- Performance: Smaller tool list for flush is acceptable / beneficial.

## Constraints

- Minimal change; no broad agent-loop refactor.
- Prefer existing `CapabilityPolicy` / registry builder patterns.

## Success Metrics

- Metric: Tools available during memory flush
- Baseline: full builtin + skills + MCP surface
- Target: memory tools only

## Rollout

- Rollout plan: merge via critical-bug automation PR.
- Rollback plan: revert the flush registry restriction if legitimate flush helpers are required later (then redesign with explicit allowlist).
