# PRD

## Problem Statement

Evolution skill synthesis trusts model JSON fields `skill.name` and `skill.entry_point` when writing files. `entry_point` is intentionally multi-segment (`scripts/main.py`), but there is no containment check after `skill_dir.join(entry_point)`. Absolute paths replace the skill root; `..` components escape it. Script content (also model-controlled, after L3/L4 gates) is then written outside `_evolved/_pending`.

## Goals

- Fail closed on path-escaping generated skill names and entry points.
- Preserve legitimate nested relative entry points used by seed prompts.
- Keep the fix local to evolution skill synthesis write/resolve helpers.

## Non-Goals

- Replacing L3/L4 content scanning.
- Fixing pending confirm/reject name validation (PR #89).
- Changing skill discovery layout.

## User Stories

- As a SkillLite user running evolution, I need generated scripts to stay inside the pending skill directory even if the model returns a malicious `entry_point`.
- As a maintainer, I need regression tests that encode absolute and traversal triggers.

## Requirements

### Functional

- Reject empty, absolute, drive-prefixed, backslash-containing, or `..`-containing entry points.
- Resolve accepted entry points strictly under the target skill directory.
- Reject generated skill names that are not a single normal path segment.

### Non-Functional

- Minimal diff; no broad refactor of skill_synth.
- No new user-facing configuration.

## Success Metrics

- Escape triggers rejected in tests.
- Valid `scripts/main.py` still accepted.
- `cargo test -p skilllite-evolution` green.

## Open Questions

- None.
