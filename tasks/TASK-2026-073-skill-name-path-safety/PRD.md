# PRD

## Summary

Reject skill directory names that are not a single safe path segment before any skills-root join, so remote skill metadata and MCP/CLI callers cannot escape the skills directory.

## Why

On `main` (`12010e8`), `skilllite add` trusts SKILL.md frontmatter `name` for the install destination (`skills_path.join(skill_name)`). MCP `get_skill_info` / `run_skill` similarly join caller-controlled `skill_name`. Concrete trigger: a remote skill with `name: ../.github/workflows/pwn` installs outside the skills root; MCP `skill_name: ../secret-skill` can read/execute outside the intended tree.

## Requirements

1. A shared validator accepts only a single `Normal` path component and rejects empty, `.`, `..`, separators, null bytes, and absolute/multi-segment forms.
2. Install/update destination selection must validate names before copy.
3. MCP and CLI lookup/remove paths must validate names before join/delete.
4. Fail closed with a clear validation error; do not silently rewrite malicious names into alternate destinations.

## Non-goals

- Changing how legitimate single-segment skill names are discovered or displayed.
- Fixing pending evolution confirm/reject in this task (tracked separately).

## Decision Log

- 2026-07-27: Place validator in `skilllite-core::path_validation` so commands and MCP share one rule.
- 2026-07-27: Prefer reject-over-rewrite for malicious frontmatter names.
