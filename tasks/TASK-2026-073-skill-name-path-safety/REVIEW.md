# REVIEW

## Findings

- Critical: `skilllite add` used SKILL.md frontmatter `name` as install destination without single-segment validation, enabling write escape via `name: ../...`.
- Critical: MCP `get_skill_info` / `run_skill` joined caller `skill_name` unsafely, enabling read/execute outside the skills root.
- Related: CLI `find_skill` / `remove` had the same join pattern and are now gated by the shared validator.
- Out of scope: pending evolution confirm/reject remains in open PR #89.

## Injected Specs Completed

- `spec/verification-integrity.md`
- `spec/task-artifact-language.md`
- `spec/security-nonnegotiables.md`
- `spec/architecture-boundaries.md`
- `spec/rust-conventions.md`
- `spec/testing-policy.md`
- `spec/docs-sync.md` (reviewed; no user-facing command/env wording change required)

## Merge readiness: ready

Minimal fail-closed fix with regression tests and validation evidence. No broad refactors.
