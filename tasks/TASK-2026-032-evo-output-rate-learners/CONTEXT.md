# Technical Context

## Audit findings (learners)

| Location | Previous hardcoded | New env (default) |
|----------|-------------------|-------------------|
| `prompt_learner` example candidate | `total_tools >= 3` (historical) | `SKILLLITE_EVO_PROMPT_EXAMPLE_MIN_TOOLS` default **2** (env-tunable) |
| `prompt_learner` rule summaries | `LIMIT 10` per bucket | `SKILLLITE_EVO_PROMPT_RULE_SUMMARY_LIMIT` = 10 |
| `memory_learner` | `-7 days`, limit 15 | `SKILLLITE_EVO_MEMORY_RECENT_DAYS`, `SKILLLITE_EVO_MEMORY_DECISION_LIMIT` |
| `skill_synth/query` | 7 days, limit 100 | `SKILLLITE_EVO_SKILL_QUERY_RECENT_DAYS`, `SKILLLITE_EVO_SKILL_QUERY_DECISION_LIMIT` |
| `query_skill_failures` | `LIMIT 5` | `SKILLLITE_EVO_SKILL_FAILURE_SAMPLE_LIMIT` |

## Observability

- `evolution_run_scope`: written after `txn_id` allocation, before snapshot; `reason` holds JSON.
- `evolution_shallow_skip`: written when shallow preflight returns; `target_id` = proposal id.
- `rule_extraction_parse_failed`: pre-existing event; documented for operators.

## Keys registry

- All new keys live under `skilllite_core::config::env_keys::evolution`; desktop `EvolutionRunEnvGuard` already forwards `SKILLLITE_EVO*`.
