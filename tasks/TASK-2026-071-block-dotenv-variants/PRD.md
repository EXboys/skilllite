# PRD

## Background

SkillLite hard-blocks agent and IDE access to `.env` so API keys and local secrets never
enter the model context or get overwritten. The implementation used `ends_with(".env")`,
which misses the filenames frameworks actually use (`.env.local`, `.env.production`).

## Objective

Close the dotenv-variant bypass so the documented "`.env` is not readable/writable"
invariant covers real project env files.

## Functional Requirements

- FR-1: Any path component named `.env`, `.envrc`, or starting with `.env.` is treated as sensitive for agent file tools.
- FR-2: Desktop IDE read/write/list uses the same rule.
- FR-3: Existing blocks for `.key`, `.pem`, and `.git/config` are unchanged.

## Non-Functional Requirements

- Security: Default is more restrictive, not more permissive.
- Performance: Lexical path-component check only; no extra filesystem I/O.
- Compatibility: Agents can no longer read/write dotenv variants that previously succeeded.

## Constraints

- Technical: Assistant crate must not take a `skilllite-core` / `skilllite-agent` dependency; duplicate the small predicate.
- Timeline: N/A (automation sweep)

## Success Metrics

- Metric: Variant paths are rejected in unit tests
- Baseline: `.env.local` read/write succeeds on main@ca126d3
- Target: `.env.local` read/write returns the existing sensitive-path error

## Rollout

- Rollout plan: merge to main
- Rollback plan: revert the predicate change
