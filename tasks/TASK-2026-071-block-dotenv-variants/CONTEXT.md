# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/builtin/helpers.rs` (`is_sensitive_write_path`)
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/workspace.rs` (`workspace_write_path_blocked`)
- Current behavior: `.env` suffix / `/.env/` directory only. `.env.local` is allowed.

## Architecture Fit

- Layer boundaries involved: agent builtin tools; assistant L3 file IO (no engine crate link)
- Interfaces to preserve: existing error strings (`Blocked: reading/writing sensitive file`)

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: previously allowed dotenv-variant IO becomes denied

## Design Decisions

- Decision: Block path components equal to `.env` / `.envrc` or starting with `.env.`
  - Rationale: Matches Vite/Next/direnv filenames without treating `src/env.rs` as sensitive
  - Alternatives considered: suffix-only `.env*` on the full path (false-positive on `environment.json` if careless); shared crate helper (assistant cannot depend on agent/core)
  - Why rejected: Assistant is CLI-bridge-only; keep a duplicated 10-line predicate

## Open Questions

- [x] Should `.env.example` be allowed? No — keep one rule; example files are still env templates.
