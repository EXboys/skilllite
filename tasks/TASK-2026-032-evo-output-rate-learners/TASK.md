# TASK Card

## Metadata

- Task ID: `TASK-2026-032`
- Title: Evolution learner tunables and observability
- Status: `done`
- Priority: `P2`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-10`
- Target milestone:

## Problem

After TASK-031, empty-run noise was reduced but **long periods without rules/skills/prompt output** remained unaddressed: hardcoded learner filters and weak audit signals made tuning and diagnosis hard.

## Scope

- In scope: Env-tunable prompt/memory/skill-synth query windows; structured `evolution_log` types for scope and shallow skip; EN/ZH docs + i18n; PRD priority lock (prompts/rules first; default `SKILLLITE_EVO_PROFILE` unchanged in code).
- Out of scope: New exploration skill generation mode; changing `should_evolve` defaults; UI charts beyond existing evolution event list.

## Acceptance Criteria

- [x] PRD records priority: **prompts/rules + learner recall** first; skills/memory via shared query envs; **no default profile change** in code.
- [x] Prompt example min tools + rule summary limit configurable; memory + skill_synth query windows configurable.
- [x] `evolution_run_scope` and `evolution_shallow_skip` logged; `rule_extraction_parse_failed` documented for operators.
- [x] EN/ZH `ENV_REFERENCE.md` + `CHANGELOG.md` + assistant event type labels updated.
- [x] `cargo test -p skilllite-evolution`, clippy, `validate_tasks.py` pass.

## Risks

- Risk: Lower `PROMPT_EXAMPLE_MIN_TOOLS` admits noisier examples
  - Impact: Lower-quality planning examples
  - Mitigation: Document trade-off; defaults unchanged.

## Validation Plan

- Required tests: `cargo test -p skilllite-evolution -p skilllite-agent`
- Commands: `cargo clippy -p skilllite-evolution -p skilllite-core -- -D warnings`, Tauri `cargo check` if assistant touched
- Manual: Optional — inspect `evolution.log` after a run for `evolution_run_scope`

## Regression Scope

- Areas: `prompt_learner`, `memory_learner`, `skill_synth/query`, `run` audit, env_keys.

## Links

- Related: [TASK-2026-031](tasks/TASK-2026-031-evo-periodic-preflight-noscope/TASK.md)
