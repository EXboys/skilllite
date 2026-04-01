# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-evolution/src/feedback.rs` (schema reference)
- Current behavior:
  - Evolution CLI supports status/reset/disable/explain/confirm/reject/run/repair-skills, but no backlog list command.

## Architecture Fit

- Layer boundaries involved:
  - Entry layer (`skilllite`) parses command and routes to command crate.
  - Command layer (`skilllite-commands`) performs read-only DB query.
- Interfaces to preserve:
  - Existing evolution subcommand signatures and output expectations.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - New subcommand only; no behavior change to existing commands.

## Design Decisions

- Decision:
  - Implement backlog query in `skilllite-commands::evolution` and return formatted table.
  - Rationale:
    - Minimal change, aligned with existing command structure.
  - Alternatives considered:
    - Add JSON output first.
  - Why rejected:
    - User requested a query command; text table is faster and lower risk for first iteration.

## Open Questions

- [ ] Should `--json` be added in a follow-up for scripting support?
- [ ] Should status/risk be enum-constrained at clap level in next iteration?
