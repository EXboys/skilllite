# PRD

## Background

Conversation-end Wiki suggestions can now prompt users to record lessons after replans or repeated tool failures. The recorded content should be actionable project knowledge: experience plus optimization guidance.

## Objective

Ensure confirmed Wiki lessons use a consistent Markdown structure that captures what happened, root cause, optimization, and next-time guidance.

## Functional Requirements

- FR-1: Suggested lessons must use an experience/optimization template.
- FR-2: `wiki record-lesson` must emit the same template when body text is omitted.
- FR-3: Recorded lesson Markdown must remain human-editable and compile through existing Repo Wiki flow.

## Non-Functional Requirements

- Security: Do not include full transcripts or secrets.
- Performance: Template generation must be deterministic and lightweight.
- Compatibility: Existing `record-lesson` callers remain valid.

## Constraints

- Technical: No new dependencies or storage backends.
- Timeline: Deterministic template only.

## Success Metrics

- Metric: Tests prove structured sections are present.
- Baseline: Proposed lesson is a short paragraph.
- Target: Suggested and recorded lessons include required headings.

## Rollout

- Rollout plan: Additive template refinement.
- Rollback plan: Callers can still pass explicit body text; existing Markdown remains valid.
