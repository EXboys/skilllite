# CONTEXT

## Technical boundaries

- Fix lives in `skilllite-agent` (`delegate_swarm`) and `skilllite` (`swarm_executor`).
- No sandbox policy change; this restores intended workspace containment for swarm paths.
- `NodeContext.workspace` remains a metadata field (path or originating node id) but is
  not trusted as a local filesystem root by `AgentTaskExecutor`.

## Compatibility notes

- Clients that previously depended on absolute cross-machine workspace paths for local
  execution were already broken on peers; node-local workspace is the compatible model.
- Tool schema keeps the `workspace` property for forward-compatible LLM prompts but marks it ignored.

## Constraints

- Minimal change: no swarm routing/auth refactors.
- Tests must not require a live swarm HTTP daemon.
