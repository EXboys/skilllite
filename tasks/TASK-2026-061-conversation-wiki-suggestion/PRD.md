# PRD

## Background

Repo Wiki dynamic refresh supports source fingerprints and command-triggered updates. The next product behavior is to let chat/Assistant capture lessons after difficult runs without silently writing wiki content.

## Objective

Provide a structured, user-confirmed path from conversation failure signals to Repo Wiki lesson ingestion.

## Functional Requirements

- FR-1: Detect suggestion-worthy conversations from structured runtime facts, especially replans and consecutive tool failures.
- FR-2: Produce a structured wiki suggestion payload for CLI/Desktop consumers.
- FR-3: Add a confirmation-time recording path that writes a compact lesson into `.skilllite/wiki/raw/` and compiles it.
- FR-4: Never write wiki content solely because the signal was emitted.

## Non-Functional Requirements

- Security: Do not include secrets or full transcripts in lessons; keep writes explicit and local.
- Performance: Suggestion building should use already-available runtime facts and short summaries.
- Compatibility: Existing chat/wiki behavior remains valid; new write path is additive.

## Constraints

- Technical: Preserve crate dependency direction and avoid new storage backends.
- Timeline: Implement deterministic trigger/recording path first; richer UI rendering can follow.

## Success Metrics

- Metric: A run with replan or three consecutive tool failures produces a suggestion, and a clean run does not.
- Baseline: No wiki suggestion signal after difficult chat runs.
- Target: Tests verify both trigger and non-trigger behavior.

## Rollout

- Rollout plan: Expose structured metadata and a record command/API; UI can opt in.
- Rollback plan: Ignore suggestion payloads or disable confirmation path without affecting existing wiki files.
