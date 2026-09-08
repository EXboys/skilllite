# PRD

## Background

SkillLite already accumulates DeepSeek `reasoning_content` in the in-memory agent loop and serializes it on outbound OpenAI-compatible requests. Evolution LLM replay has the same contract. Chat transcripts do not store the field, so a new process (or the next CLI/desktop turn) rebuilds history without it.

DeepSeek thinking mode returns HTTP 400 when a request includes `tools` and prior assistant messages omit `reasoning_content`. SkillLite agent turns send tools by default.

## Objective

- Persist last-turn assistant `reasoning_content` on transcript `Message` rows.
- Restore it on history reload so subsequent turns echo the field.

## Functional Requirements

- FR-1: When an assistant turn has `reasoning_content`, the transcript `Message` row stores it.
- FR-2: Reloaded assistant `ChatMessage` values include that field.
- FR-3: Rows written before this change deserialize with `reasoning_content = None`.

## Non-Functional Requirements

- Security: No new path or auth surface.
- Performance: One optional string on assistant rows only.
- Compatibility: Additive JSON field; serde default.

## Constraints

- Technical: Minimal change; no transcript format version bump.
- Timeline: Same automation run as the 2026-09-08 critical sweep.

## Success Metrics

- Metric: Reloaded history used for a tools-enabled follow-up includes prior assistant `reasoning_content`.
- Baseline: Field always `None` after `read_history`.
- Target: Field matches the value persisted from the previous turn.

## Rollout

- Rollout plan: Merge the additive transcript field; old files remain readable.
- Rollback plan: Revert the commit; leftover `reasoning_content` keys are ignored by older binaries if they use default serde ignore on unknown fields within the variant (they will fail if the variant is strictly deny-unknown — current schema does not deny unknown fields).
