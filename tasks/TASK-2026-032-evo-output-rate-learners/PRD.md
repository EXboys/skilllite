# PRD

## Background

Users can go long stretches without seeing rules, skills, or prompt artifacts evolve. Causes span proposal gates (separate work), **learner input filters**, and **limited observability** into which stage blocked output.

## Objective

Increase **operational control** over learner recall (configurable windows) and **audit clarity** (structured log events) without changing default runtime behavior when env vars are unset.

## Priority lock (product)

1. **Primary**: Prompts/rules path — `prompt_learner` example and rule-summary limits (highest user-visible impact for “no rules”).
2. **Secondary**: Memory and skill-synth SQL windows (same release; shared pattern).
3. **Explicit non-change**: Default **`SKILLLITE_EVO_PROFILE`** remains unset/`default` in code; documentation continues to recommend `demo` for aggressive tuning.

## Functional Requirements

- FR-1: Env vars control prompt example min tools and rule extraction row limits (defaults = previous hardcoded values).
- FR-2: Env vars control memory learner lookback and row cap.
- FR-3: Env vars control skill-synth query lookback, scan cap, and per-skill failure sample size.
- FR-4: Each full evolution execution logs `evolution_run_scope` with scope JSON; shallow skip logs `evolution_shallow_skip` before outcome duplication.
- FR-5: Document `rule_extraction_parse_failed` and new event types in EN/ZH env reference; map labels in assistant UI.

## Non-Functional Requirements

- Security: No relaxation of gatekeeper L3/L1; only query windows and audit metadata.
- Performance: Larger limits may increase LLM prompt size — caps documented.
- Compatibility: Defaults preserve historical SQL behavior.

## Success Metrics

- Operators can cite audit rows to distinguish “no scope” vs “shallow skip” vs “parse failed” vs “empty LLM output.”
- Support can recommend a small set of env tweaks for “light tasks” (e.g. `PROMPT_EXAMPLE_MIN_TOOLS=1`) without code changes.
