# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/growth.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - `crates/skilllite-evolution/src/growth_schedule.rs` (`growth_due` vs `inspect_growth_due`)
- Current behavior:
  - Life Pulse stores `last_periodic_growth_unix: Mutex<Option<i64>>` starting at `None`.
  - `evolution_growth_due` reads it, passes it to `skilllite evolution status --json --periodic-anchor-unix` only when `Some`, and never writes it back.
  - `inspect_growth_due` uses `anchor_eff = last.unwrap_or(now_unix)`, so a persistent `None` makes `periodic_elapsed_secs` ~0 every tick.

## Architecture Fit

- Layer boundaries involved: Assistant L2 bridge (CLI JSON only) vs in-process `ChatSession` A9 timer.
- Interfaces to preserve: `EvolutionStatusPayload.a9` fields (`arm_periodic`, `growth_tick_would_be_due`, `periodic_only`) and `--periodic-anchor-unix`.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: First desktop heartbeat after upgrade begins the interval clock instead of leaving it unset. No CLI/API change.

## Design Decisions

- Decision: Apply the same mutation rules as `growth_due` in the assistant after inspect, without calling `growth_due` in-process.
  - Rationale: L2 split forbids an evolution-engine dependency; status JSON already exposes `arm_periodic`.
  - Alternatives considered: Call `growth_due` from the Tauri crate; persist the anchor only after a successful spawn.
  - Why rejected: Engine dependency violates L2; spawn-only writes would still leave `None` forever because the first inspect never reports due.

## Open Questions

- [x] Should a skipped periodic-only/no-proposal tick advance the anchor? Yes — matches `chat_session` (mutate first, then skip spawn).
