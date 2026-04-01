# Context: 子 crate 错误类型统一

## Current State

| Crate | thiserror Error | anyhow 使用量 |
|-------|-----------------|--------------|
| skilllite (CLI) | `Error` (完善) | 6 |
| skilllite-core | `PathValidationError` (窄) | 14 |
| skilllite-executor | `ExecutorError` (窄) | 7 |
| skilllite-sandbox | `BashValidationError` (窄) | 59 |
| skilllite-agent | 声明未使用 | 98 |
| skilllite-commands | 无 | 84 |
| skilllite-evolution | 无 | 51 |
| skilllite-fs | 无 | 18 |
| skilllite-swarm | 无 | 10 |

## Dependency Direction (bottom-up migration order)

```
skilllite-fs → skilllite-core → skilllite-sandbox → skilllite-executor
                                                  → skilllite-evolution
                                                  → skilllite-swarm
                                                  → skilllite-agent → skilllite-commands → skilllite (CLI)
```

## Technical Constraints

- `#[from] anyhow::Error` 每个 Error enum 最多有一个（Rust orphan rule）
- 已有的窄错误类型（PathValidationError 等）被外部 crate 直接引用，需保持 re-export
- `skilllite-assistant` (Tauri) 被 workspace exclude，不在本次范围
