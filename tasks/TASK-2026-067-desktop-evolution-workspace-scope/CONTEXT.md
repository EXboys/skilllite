# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/chat.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/evolution_cli.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-core/src/paths.rs`
- Current behavior:
  - Desktop chat sets subprocess cwd to the project root and sends `config.workspace`, but does not set `SKILLLITE_WORKSPACE`; `ChatSession` stores state under `skilllite_executor::chat_root()`.
  - `evolution run --workspace` loads dotenv, then sets `SKILLLITE_WORKSPACE` to the resolved workspace before calling `paths::chat_root()`.
  - Desktop pending/confirm commands use `resolve_skills_dir_with_legacy_fallback(root, "skills")`, while `evolution run` uses `<workspace>/.skills`.

## Architecture Fit

- Layer boundaries involved: desktop Tauri bridge -> `skilllite` CLI entry -> `skilllite-commands` -> evolution/agent crates.
- Interfaces to preserve: existing CLI subcommands and JSON shapes remain unchanged.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: legacy `.skills/` remains supported when no `skills/` directory exists.

## Design Decisions

- Decision: Inject absolute `SKILLLITE_WORKSPACE` in desktop subprocess helpers after merging dotenv/UI overrides.
  - Rationale: The desktop caller passes a workspace explicitly; env-dependent storage roots must match that explicit contract.
  - Alternatives considered: Change `chat_root()` to inspect cwd or `AgentConfig.workspace`.
  - Why rejected: That would affect non-desktop callers and broaden the blast radius.
- Decision: Reuse `resolve_skills_dir_with_legacy_fallback` in `evolution run`.
  - Rationale: It aligns run output with pending/confirm/status and preserves legacy fallback.
  - Alternatives considered: Change desktop pending/confirm to `.skills`.
  - Why rejected: That would regress OpenClaw/default `skills/` workspaces.

## Open Questions

- [x] Is this a security bypass? No; the bug is state partitioning and wrong root selection, while gatekeeper constraints remain intact.
- [x] Are docs required? No user-facing flags or documented commands change; behavior is corrected to the existing workspace contract.
