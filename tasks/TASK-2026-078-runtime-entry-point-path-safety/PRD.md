# PRD

## Summary

Runtime skill execution must treat `entry_point` as a skill-relative path only. Absolute, traversal, drive-prefix, and backslash forms must be rejected before the interpreter is spawned.

## Why

Untrusted or compromised skill metadata (or LLM entry-point inference) can currently point `run` at host files outside the skill directory. At sandbox level 1 this is arbitrary code execution as the SkillLite user.

## Requirements

1. Lexically validate entry points before join: reject empty/null, `\`, Windows drive prefixes, absolute paths, `..`, and non-normal components.
2. Keep nested relatives (`scripts/main.py`, `./main.py`) working.
3. Apply validation at metadata acceptance, CLI/agent overrides, and a final pre-run gate.
4. Prefer fail-closed errors over silently executing escaped paths.

## Non-goals

- Changing sandbox level semantics
- Hardening evolution synthesis writes (covered by #127)
- Symlink-follow hardening for builtin file tools
