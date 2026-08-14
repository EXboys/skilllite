# TASK Card

## Metadata

- Task ID: TASK-2026-071
- Title: Block dotenv variant files from agent and IDE IO
- Status: `done`
- Priority: `P0`
- Owner: agent
- Contributors: agent
- Created: 2026-08-14
- Target milestone: next patch

## Problem

Agent `read_file` / `write_file` and desktop IDE workspace IO treat `.env` as a hard-blocked
sensitive path, but the check only matches an exact `.env` suffix or a `/.env/` directory.
Common dotenv variants (`.env.local`, `.env.production`, `.env.development`, `.envrc`) are
readable and writable. In a Vite/Next.js workspace this leaks `DATABASE_URL` and similar
secrets to the LLM, or lets the agent overwrite local env files (data loss).

## Scope

- In scope:
  - Agent builtin sensitive-path check used by read/write/edit tools
  - Desktop IDE workspace read/write/list sensitive-path check
  - Regression tests
  - EN/ZH wording for the blocked set
- Out of scope:
  - ChatSession workspace-root alignment (#114)
  - Evolution restore atomicity (#142)
  - Symlink canonicalize (#132)
  - Broad redaction-key expansion

## Acceptance Criteria

- [x] `.env.local`, `.env.production`, `.envrc` are blocked for agent read and write
- [x] Existing `.env`, `.key`, `.pem`, `.git/config` blocks still hold
- [x] Non-env files such as `src/env.rs` and `environment.json` remain allowed
- [x] Desktop IDE uses the same dotenv-variant rule
- [x] Tests fail if the variant check is removed

## Risks

- Risk: Blocking `.env.example` prevents the agent from reading committed templates
  - Impact: Minor UX; agent cannot open example env files
  - Mitigation: Acceptable; example files follow the same dotenv naming contract

## Validation Plan

- Required tests: `cargo test -p skilllite-agent`, assistant workspace unit tests
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p skilllite-agent`, `cargo test -p skilllite-assistant --lib` (if deps allow) or the workspace.rs unit tests
- Manual checks: N/A

## Regression Scope

- Areas likely affected: agent file tools, desktop IDE file tree / editor
- Explicit non-goals: sandbox seatbelt deny lists, run_command regex (already matches `.env` as a substring)

## Links

- Source TODO section: critical-bug automation 2026-08-14
- Related PRs/issues: none on main; related backlog #132 (symlinks)
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
