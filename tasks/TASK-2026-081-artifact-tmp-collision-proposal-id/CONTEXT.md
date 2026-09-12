# CONTEXT — TASK-2026-081

## Technical boundaries

- Crate: `skilllite-artifact` (`local_dir.rs` only for the store fix).
- Crate: `skilllite-evolution` (`scope.rs` proposal ID construction).
- No public API signature changes expected.
- No DB schema migration: `proposal_id` remains a free-form unique string.

## Constraints

- Avoid adding new production dependencies to `skilllite-artifact` if a std-only unique temp name is sufficient.
- `skilllite-evolution` already depends on `uuid` — prefer UUID (or timestamp + UUID) for proposal IDs.
- Keep English task artifacts per `spec/task-artifact-language.md`.

## Compatibility notes

- Proposal ID format will grow (timestamp + uniqueness suffix). Treat as opaque.
- Artifact on-disk layout of final keys is unchanged; only staging temp filenames change.

## Related open work (not in scope)

- PR #123: Windows artifact key escapes (touches `local_dir.rs` validation, not temp naming).
- PR #117 / #90: authorized proposal routing (different bug class).
- PR #112–#132: other critical-path sweeps still open against `main` @ `12010e8`.
