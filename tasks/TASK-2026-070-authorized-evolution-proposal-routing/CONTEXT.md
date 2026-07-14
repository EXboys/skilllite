# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-evolution/src/run.rs`
- Current behavior: The desktop bridge sets `SKILLLITE_EVO_FORCE_PROPOSAL_ID` on the child process,
  but `cmd_run` removes it whenever the parsed CLI `proposal_id` is `None`.

## Architecture Fit

- Layer boundaries involved: Desktop entry bridge invokes the existing CLI commands layer.
- Interfaces to preserve: `evolution run --proposal-id`, workspace routing, and JSON output.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: No public interface changes; explicit CLI routing replaces an
  ineffective inherited environment value for this path.

## Design Decisions

- Decision: Add `--proposal-id` and its value to `authorized_evolution_run_args`.
  - Rationale: The CLI argument is the commands layer's source of truth and matches the existing
    manual desktop trigger implementation.
  - Alternatives considered: Preserve inherited environment state inside `cmd_run`.
  - Why rejected: That would weaken explicit invocation isolation and could make unrelated callers
    accidentally force a proposal from stale process state.

## Open Questions

- [x] Does the CLI already support explicit proposal selection? Yes.
- [x] Are docs changes required? No; this restores intended behavior without changing interfaces.
