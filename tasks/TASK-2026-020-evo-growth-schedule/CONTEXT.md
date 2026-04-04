# CONTEXT

## Crates

- `skilllite-evolution`: new `growth_schedule.rs`; exports `GrowthScheduleConfig`, `growth_due`, `signal_burst_due`, `weighted_unprocessed_signal_sum`, `seconds_since_last_evolution_run`.
- `skilllite-core`: `env_keys::evolution` extended.
- `skilllite-agent` (`chat_session.rs`): shared `A9_LAST_PERIODIC_GROWTH_UNIX`; periodic loop **first tick without prior sleep**; `maybe_trigger_evolution_*` uses `signal_burst_due`.
- `skilllite-assistant` Tauri `integrations.rs`: `growth_schedule_merged_for_workspace`; `evolution_growth_due` uses `growth_due`; `EvolutionStatusPayload` extended.
- Frontend `EvolutionSection.tsx`, i18n, `useSettingsStore` copy for default interval.

## Compatibility

- Workspaces with `.env` still setting `SKILLLITE_EVOLUTION_INTERVAL_SECS=1800` or `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` behave per explicit env; only **unset** defaults changed (interval 600, weighted arm added).
- `collect_active_scope` no longer reads `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`; operators needing the old coupling can set `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS` to match.

## SQLite

- `seconds_since_last_evolution_run` uses `Option<i64>` column read because `MAX(ts)` is NULL when no matching log rows.
