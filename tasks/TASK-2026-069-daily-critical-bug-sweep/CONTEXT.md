# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`, `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/growth.rs`, `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/mod.rs`, `skilllite/src/cli.rs`, `crates/skilllite-commands/src/evolution.rs`, `crates/skilllite-commands/src/schedule.rs`.
- Current behavior: current branch `cursor/critical-bug-investigation-d75f` contains a fix for a recent desktop Life Pulse regression introduced by the CLI-only bridge refactor.

## Architecture Fit

- Layer boundaries involved: desktop assistant bridge spawning CLI subprocesses; engine CLI remains the execution boundary.
- Interfaces to preserve: Life Pulse still delegates heavy work to `skilllite` CLI; no direct engine dependency is reintroduced.

## Dependency and Compatibility

- New dependencies: none planned.
- Backward compatibility notes: existing Life Pulse UI and CLI arguments are preserved; background subprocesses now receive the same workspace contract as manual desktop actions.

## Design Decisions

- Decision: perform a review-only audit first and defer implementation until a concrete critical trigger scenario is established.
  - Rationale: the automation explicitly requires a high confidence bar and no PR for doubtful findings.
  - Alternatives considered: proactively patch suspicious code based on pattern matching.
  - Why rejected: it risks false positives and unnecessary behavior drift.
- Decision: fix Life Pulse by adding a small command builder that sets `current_dir`, `--workspace`, and absolute `SKILLLITE_WORKSPACE` consistently for background growth/rhythm subprocesses.
  - Rationale: manual desktop actions already use this workspace contract, and the bug was caused by background subprocesses omitting it.
  - Alternatives considered: call the in-process engine directly from the desktop bridge.
  - Why rejected: it would reverse the CLI-only bridge direction from the recent refactor.
- Decision: update the desktop periodic anchor when the first successful check initializes it and when the periodic arm fires.
  - Rationale: this restores the mutation that `growth_due` performed before the read-only CLI status path was introduced.
  - Alternatives considered: only update after the subprocess exits successfully.
  - Why rejected: the original semantics advance the periodic anchor when the periodic arm fires, even if a periodic-only preflight later skips due to no proposals.

## Open Questions

- [x] Which recent commits contain high-blast-radius behavioral changes?
- [x] Does any suspicious change have a concrete trigger scenario with critical impact?
