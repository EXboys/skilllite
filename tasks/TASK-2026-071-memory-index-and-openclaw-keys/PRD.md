# PRD

## Background

Daily high-severity sweep on 2026-08-26 confirmed two review-escaped correctness bugs: session-clear memory is indexed under the wrong SQLite file, and OpenClaw secret migration assigns every provider API key to `OPENAI_API_KEY`.

## Objective

- Cleared-session summaries must be searchable by the same FTS index the agent already queries.
- OpenClaw provider secrets must land in the matching allowlisted environment variable.

## Functional Requirements

- FR-1: Session-clear memory indexing uses agent id `default` (same as `execute_memory_tool` and `build_memory_context`).
- FR-2: OpenClaw `models.providers.<id>.apiKey` maps to the allowlisted key for that provider id.
- FR-3: Unknown provider ids are skipped (no write to `OPENAI_API_KEY`).
- FR-4: Existing allowlisted `.env` / `env` object merge is unchanged.

## Non-Functional Requirements

- Security: Do not copy non-OpenAI secrets into `OPENAI_API_KEY`.
- Performance: No change; same single-file FTS write.
- Compatibility: Orphan `{session_key}.sqlite` files are left unused (already unused by search).

## Constraints

- Technical: Minimal patch; no memory-architecture redesign.
- Timeline: Same sweep PR.

## Success Metrics

- Metric: Regression tests cover both triggers.
- Baseline: Named-session clear indexes `{session_key}.sqlite`; provider keys all become `OPENAI_API_KEY`.
- Target: Shared `default.sqlite`; mapped keys only.

## Rollout

- Rollout plan: Merge the sweep PR.
- Rollback plan: Revert the commit.
