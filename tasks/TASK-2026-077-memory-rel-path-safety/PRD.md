# PRD

## Background

Memory file writes via executor RPC interpolate `rel_path` into
`chat_root/memory/{rel_path}`. The current guard rejects `..` and leading `/`
but still accepts Windows drive-absolute and backslash-rooted forms. On Windows
hosts those values escape the memory directory through `Path::join` replacement
semantics, enabling arbitrary file create/overwrite under process permissions.

## Objective

- Reject path-escaping memory `rel_path` values before any filesystem mutation.
- Keep legitimate nested relative notes (e.g. `MEMORY.md`, `notes/day.md`) working.

## Functional Requirements

- FR-1: Reject empty, NUL, `..`, absolute, leading `\` / `/`, backslash separators, and Windows drive prefixes (string-level, host-independent).
- FR-2: After join, lexically normalize and require the result stays under the memory directory.
- FR-3: `memory_write` must fail closed with a validation error for unsafe paths.
- FR-4: Reindex helpers must skip the same unsafe relative paths.

## Non-Functional Requirements

- Security: fail-closed; no parent directory creation for escaped paths.
- Compatibility: existing `/`-separated relative notes unchanged.
- Docs: no user-facing path grammar docs required.

## Non-Goals

- Fixing agent workspace symlink follow
- Merging or rebasing open PR #128 agent_id work in this change
