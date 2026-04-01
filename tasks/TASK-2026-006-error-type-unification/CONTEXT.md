# Context: Unify Sub-crate Error Types

## Current State

| Crate | thiserror Error | anyhow usage count |
|-------|-----------------|--------------|
| skilllite (CLI) | `Error` (mature) | 6 |
| skilllite-core | `PathValidationError` (narrow) | 14 |
| skilllite-executor | `ExecutorError` (narrow) | 7 |
| skilllite-sandbox | `BashValidationError` (narrow) | 59 |
| skilllite-agent | none declared | 98 |
| skilllite-commands | none | 84 |
| skilllite-evolution | none | 51 |
| skilllite-fs | none | 18 |
| skilllite-swarm | none | 10 |

## Dependency Direction (bottom-up migration order)

```
skilllite-fs → skilllite-core → skilllite-sandbox → skilllite-executor
                                                  → skilllite-evolution
                                                  → skilllite-swarm
                                                  → skilllite-agent → skilllite-commands → skilllite (CLI)
```

## Technical Constraints

- Each `Error` enum can only have one `#[from] anyhow::Error` conversion to avoid conflicting impls.
- Existing narrow error types (such as `PathValidationError`) are directly referenced by other crates and must remain re-exported.
- `skilllite-assistant` (Tauri) is excluded from the workspace and out of scope.
