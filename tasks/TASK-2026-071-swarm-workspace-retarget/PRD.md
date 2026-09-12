# PRD

## Summary

Prevent swarm-delegated and locally routed swarm tasks from treating untrusted
`workspace` strings as filesystem roots for agent tool containment.

## Goals

- Close the containment bypass where LLM/API-chosen workspace retargets `write_file`.
- Keep swarm delegation usable without requiring callers to pass workspace.

## Non-Goals

- Redesign P2P NodeContext semantics beyond clarifying trust boundaries.
- Change swarm authentication or capability routing.

## Requirements

1. `delegate_to_swarm` MUST always set `NodeTask.context.workspace` to the calling agent's workspace.
2. Local `AgentTaskExecutor` MUST resolve execution workspace from node configuration (`--skills-dir` parent or cwd), not from the task payload.
3. Override attempts SHOULD be logged at warn level for auditability.
4. Behavior MUST be covered by unit regression tests.

## User-visible impact

- Tool parameter `workspace` on `delegate_to_swarm` is ignored (documented in tool schema).
- HTTP `POST /task` with a crafted `context.workspace` no longer retargets local FS roots on agent-backed nodes.
