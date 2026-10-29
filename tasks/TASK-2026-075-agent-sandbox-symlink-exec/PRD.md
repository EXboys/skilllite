# PRD

## What

`skilllite_sandbox::run` must start the official Codex standalone binary. That binary is reached through more than one symlink. The sandbox executes the canonical file and keeps the original symlink directory on `PATH`.

## Why

Executing the symlink path fails before Codex can run, because bubblewrap does not recreate the intermediate `current` symlink. Users who installed Codex with the standalone installer cannot use `--runtime codex`.

## Decisions

- Decision: `canonicalize` the executable and pass that path to bwrap or `sandbox-exec`.
  - Why: The final directory is already mounted. The canonical path exists there. The symlink path does not.
- Decision: When the symlink's parent differs from the canonical parent, mount that parent read-only and prepend it to `PATH`.
  - Why: npm-style `codex.js` uses `#!/usr/bin/env node`, and `node` often sits next to the symlink rather than next to the real script.
- Decision: Do not mount the whole `~/.codex` tree.
  - Why: That directory holds `auth.json` and other subscription state. The run already copies the subscription directory into the ephemeral home.

## Out of scope

Auth writeback behavior, proxy nftables, and the connect service.
