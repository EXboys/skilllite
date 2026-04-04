# PRD

## Problem

A9 evolution autorun used a coarse **periodic interval + raw unprocessed decision count**, with the same env var also gating **active** proposal construction. Users wanted **faster, more directed** triggers (weighted recent signals, sliding window), **sweep** for light usage, **min gap** between runs, and **decoupled** thresholds for active scope vs spawn.

## Decisions

- Centralize spawn/due logic in `skilllite-evolution::growth_schedule` so **desktop Life Pulse**, **status API**, and **`ChatSession`** stay aligned.
- Default **periodic interval 600s** (10 min); **weighted** sum over latest meaningful unprocessed decisions (default ≥ **3**, window **10**; failures / `feedback = neg` count double).
- Keep **OR** raw unprocessed row count ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` (default **10**).
- **Sweep**: if last `evolution_run` older than `SKILLLITE_EVO_SWEEP_INTERVAL_SECS` (default 24h) and weighted ≥ **1**, due.
- **`SKILLLITE_EVO_MIN_RUN_GAP_SEC`**: optional throttle between `evolution_run` logs (default **0** = off).
- **Active** proposals: `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS` (default **10**), separate from A9 spawn.

## Out of scope (this task)

- Two-phase LLM “plan then load full artifacts” inside `run_evolution` learners.
- Separate daily **plan vs execute** token budgets beyond existing `SKILLLITE_MAX_EVOLUTIONS_PER_DAY` / coordinator budgets.
- Feeding last-K evolution summaries into learner prompts (future prompt work).

## Success

- Defaults and env keys documented in EN/ZH `ENV_REFERENCE.md` and `CHANGELOG.md`.
- Tests cover weighted sum, periodic anchor, signal burst.
- Tauri + agent compile clean.
