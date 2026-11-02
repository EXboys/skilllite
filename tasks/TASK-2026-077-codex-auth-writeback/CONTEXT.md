# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `skilllite/src/cli.rs`, EN/ZH architecture and home-node docs.
- Current behavior before the fix: `persist_codex_auth` copied the ephemeral regular file to `.codex/.auth.json.tmp` and renamed it onto `auth.json` whenever the ephemeral file existed.

## Architecture Fit

- Layer boundaries involved: behavior stays inside `skilllite-sandbox`. The CLI flag text matches the library.
- Interfaces to preserve: `RunRequest`, `AgentRuntime`, and `run` signatures. The directory `flock` on `account_dir` still covers only other SkillLite runs.

## Dependency and Compatibility

- New dependencies: none. The lock uses the existing `libc` `flock`.
- Backward compatibility notes: a Codex run that does not modify `auth.json` no longer changes the host file's bytes or mtime. A run that rewrites the file still replaces it.

## Design Decisions

- Decision: compare the exit bytes with the startup snapshot and skip writeback when they are equal.
  - Rationale: the host Codex process is the normal concurrent writer. The subscription directory lock does not include it. Writing the unchanged snapshot back reverts that writer.
  - Alternatives considered: hold `auth.lock` for the whole sandbox run.
  - Why rejected: an agent run can last long enough to block every Codex save on that directory.
- Decision: `flock` `auth.lock` only around a real writeback.
  - Rationale: Codex serializes its own read-modify-write on that file. Matching the lock avoids interleaving the rename with Codex's commit.
  - Alternatives considered: compare-and-swap that refuses to write when the host bytes also changed.
  - Why rejected: if the sandbox rotated the refresh token, dropping the writeback leaves the revoked snapshot on the host. That conflict stays the previous last-writer behavior.

## Open Questions

- [x] Resolved: unchanged bytes are not a refresh and must not be written.
- [x] Resolved: two different refreshes in one run are out of scope for this fix.
