# PRD

## Summary

Fail closed when OpenClaw import skill destination names can escape the skills root via `Path::join`.

## Why

Untrusted OpenClaw skill frontmatter can choose the install destination name. Without single-segment validation, import becomes an arbitrary directory write under/outside the skills tree.

## Requirements

1. Destination names must be a single normal path segment before any skills-root join.
2. Unsafe names are skipped with a visible warning; they must not abort safe siblings unless no work remains.
3. Install joins must go through `skill_dir_under_root` as defense in depth.

## Non-requirements

- User-facing docs/command help changes (behavior is strictly narrower: reject previously unsafe names).
- Fixing other skill-name join sites already covered by PR #125.
