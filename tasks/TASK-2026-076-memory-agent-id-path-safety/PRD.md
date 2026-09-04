# PRD

## Background

Recent critical sweeps fixed path escapes for artifact keys, skill directory
names, session keys, and evolution entry points. Memory SQLite indexing still
interpolates caller-controlled `agent_id` into a filesystem join with no
single-segment check, so stdio RPC can write indexes outside the chat root.

## Objective

- Reject path-escaping memory `agent_id` values before SQLite open/create.
- Keep normal IDs such as `default` working under `<chat_root>/memory/`.

## Functional Requirements

- FR-1: `agent_id` must be a single normal path segment (no `/`, `\`, `..`, drive prefix, NUL).
- FR-2: `index_path` must validate before joining under `memory/`.
- FR-3: Executor `memory_write` / `memory_search` must return validation errors for bad IDs.
- FR-4: Agent memory helpers that call `index_path` must surface the same failures.

## Non-Functional Requirements

- Security: fail-closed; do not create parent directories for escaped paths.
- Performance: validation is O(n) string/component checks only.
- Compatibility: existing single-segment IDs unchanged.

## Non-Goals

- Redesigning multi-agent memory sharding
- Hardening memory `rel_path` beyond existing checks in this PR
