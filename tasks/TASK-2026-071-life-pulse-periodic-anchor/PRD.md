# PRD

## Background

A9 growth is due when any of periodic interval, weighted signals, raw backlog, or sweep fire. Chat/agent-rpc uses mutating `growth_due`, which seeds and refreshes `last_periodic_spawn_unix`. Desktop Life Pulse was split to L2 CLI status (`inspect_growth_due`) and kept a mutex for `--periodic-anchor-unix`, but never writes that mutex. Documented “every `SKILLLITE_EVOLUTION_INTERVAL_SECS`” therefore never starts on the desktop heartbeat.

## Objective

Restore advertised Life Pulse periodic evolution without pulling the evolution engine into the assistant crate.

## Functional Requirements

- FR-1: After a successful status inspect, persist a first-seen `None` anchor as `now_unix`.
- FR-2: When inspect reports `arm_periodic`, persist `now_unix` even if the spawn is skipped for empty proposals.
- FR-3: Signal-only or sweep-only dues must not rewrite the periodic anchor.
- FR-4: Spawn policy remains: skip disabled mode, skip when not due, skip periodic-only with no proposals.

## Non-Functional Requirements

- Security: No change to sandbox, confirmation, or authorization gates.
- Performance: One extra mutex write per heartbeat that already spawned `evolution status`.
- Compatibility: L2 CLI contract (`--periodic-anchor-unix`) unchanged.

## Constraints

- Technical: Assistant crate must stay CLI-JSON only (no `skilllite-evolution` dependency).
- Timeline: Minimal fix for the scheduled high-severity sweep.

## Success Metrics

- Metric: Periodic arm can become due after `interval_secs` on the desktop heartbeat path.
- Baseline: `arm_periodic` is always false while the mutex stays `None`.
- Target: Mutex is `Some(now)` after the first successful inspect and refreshes when `arm_periodic` is true.

## Rollout

- Rollout plan: Merge the targeted assistant change; no env or CLI flag change.
- Rollback plan: Revert the commit; Life Pulse returns to inspect-only (periodic arm stays dead, signal/sweep unchanged).
