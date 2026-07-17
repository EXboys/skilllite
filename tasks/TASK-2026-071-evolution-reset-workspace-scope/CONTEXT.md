# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-core/src/skill/discovery.rs`
  - `skilllite/tests/cli_evolution_workspace.rs`
- Current behavior:
  - `cmd_run` uses the requested workspace's `<workspace>/chat` and effective
    `skills/` directory.
  - `cmd_reset` uses `paths::chat_root()` and hard-coded `.skills/`, which can
    target two different workspaces and miss modern evolved skills.

## Architecture Fit

- Layer boundaries involved: CLI entry -> commands -> evolution/core helpers.
- Interfaces to preserve: Existing command output and reset confirmation gate.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Existing `.skills/` projects continue to work.
  The no-flag command follows the project-local `--workspace .` convention.

## Design Decisions

- Decision: Pass the CLI workspace into `cmd_reset` and derive chat and skill
  paths from the existing explicit workspace helpers.
  - Rationale: A destructive operation needs one explicit scope and should
    share path semantics with the operation that created the data.
  - Alternatives considered: Continue consulting `SKILLLITE_WORKSPACE` when
    the flag is omitted.
  - Why rejected: An inherited environment variable can silently redirect a
    destructive command away from the user's current project.

## Open Questions

- [x] Should legacy `.skills/` remain supported? Yes, as a fallback only.
- [x] Should unrelated `disable`/`explain` behavior change? No.
