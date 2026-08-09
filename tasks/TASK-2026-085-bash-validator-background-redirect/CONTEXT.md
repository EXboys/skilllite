# CONTEXT

## Technical boundaries

- Change is confined to `crates/skilllite-sandbox/src/bash_validator.rs` plus EN/ZH architecture notes.
- Call sites in `skilllite-agent` / `skilllite-commands` already call `validate_bash_command` before `sh -c`; no call-site API change.

## Constraints

- Keep fail-closed substring policy consistent with existing operators.
- Do not relax blocked-prefix or allowed-pattern checks.
- Avoid broad refactors in the same PR.

## Compatibility notes

- Stricter than `main@12010e8`: previously accepted commands with `&` / `>` / `<` (including URL query strings containing `&`) will now fail validation.
- This is intentional security hardening for an unsandboxed execution path.

## Near-misses deferred

- `SilentEventSink` auto-approves `ConfirmRequired` during memory flush / swarm single-task.
- `ChatSession` uses `chat_root()` and ignores `config.workspace` when `SKILLLITE_WORKSPACE` is unset.
- Concurrent `sessions.json` RMW last-writer-wins (medium metadata loss).
