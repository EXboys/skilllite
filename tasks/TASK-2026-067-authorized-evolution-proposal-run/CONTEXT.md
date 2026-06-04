# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`, `crates/skilllite-commands/src/evolution.rs`, `crates/skilllite-evolution/src/run.rs`.
- Current behavior: `authorize.rs` enqueues a proposal, spawns `skilllite evolution run --json`, sets `SKILLLITE_EVO_FORCE_PROPOSAL_ID`, and discards output. `cmd_run` removes that env var when no `--proposal-id` argument is supplied, so `run_evolution_inner` does not load the authorized proposal.

## Architecture Fit

- Layer boundaries involved: desktop assistant UI bridge -> SkillLite CLI -> command crate -> evolution engine.
- Interfaces to preserve: Tauri command shape, `skilllite evolution authorize-capability`, `skilllite evolution run --proposal-id`, and workspace dotenv merging.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: existing callers that set `--proposal-id` are unchanged; this fix aligns the authorization background caller with the supported CLI argument.

## Design Decisions

- Decision: pass `--proposal-id` in the background subprocess argument list and keep env propagation harmless.
  - Rationale: `cmd_run` treats the CLI argument as the authoritative way to force a proposal and scopes the environment variable safely around execution.
  - Alternatives considered: modify `cmd_run` to preserve a pre-existing env var when no CLI argument is passed.
  - Why rejected: preserving ambient env would broaden the force semantics for all CLI callers and risk unexpected forced runs from inherited process environments.

## Open Questions

- [x] Is a docs update required? No, this restores the existing documented command behavior and does not change user-facing command syntax.
- [x] Is a database migration required? No.
