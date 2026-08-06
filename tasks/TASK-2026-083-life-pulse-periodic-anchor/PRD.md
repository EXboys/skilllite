# PRD — Life Pulse periodic growth anchor

## Problem

After the CLI-only Life Pulse split, desktop interval-based evolution growth never becomes due: the host passes a permanent `None` periodic anchor into `inspect_growth_due`, which treats elapsed time as zero.

## Goals

- Restore first-tick seed + periodic-arm advance semantics equivalent to `growth_due` for Life Pulse.
- Keep status inspection CLI-only (no in-process DB mutation in the Tauri host beyond the mutex).

## Non-Goals

- Redesigning A9 schedule configuration.
- Changing agent-rpc in-process timers.

## Success Metrics

- With Life Pulse enabled and no signal/sweep arms, after `SKILLLITE_EVOLUTION_INTERVAL_SECS` the periodic arm can become true and the mutex advances so the next interval starts fresh.
