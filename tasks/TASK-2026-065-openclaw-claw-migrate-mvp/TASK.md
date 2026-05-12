# TASK Card

## Metadata

- Task ID: `TASK-2026-065`
- Title: CLI: claw migrate MVP from OpenClaw
- Status: `done`
- Priority: `P1`
- Owner: `maintainer`
- Contributors:
- Created: `2026-05-12`
- Target milestone:

## Problem

Users moving from OpenClaw / legacy Clawdbot layouts needed a single migration entry point beyond `import-openclaw-skills`, covering persona/memory Markdown and optional API keys with preview, backup, and a report directory.

## Scope

- In scope: `skilllite claw migrate` and `skilllite migrate openclaw`; skills via existing import pipeline; SOUL/MEMORY/USER/daily memory Markdown; allowlisted `.env` merge with `--migrate-secrets`; `--dry-run`, default backup zip, `--skill-conflict`, archive of unmapped OpenClaw config and workspace instruction files; report under `.skilllite/migration/openclaw/<timestamp>/`.
- Out of scope: Hermes-specific paths, session/transcript import, full `openclaw.json` field mapping, messaging/TTS/cron migration.

## Acceptance Criteria

- [x] `skilllite claw migrate --dry-run` prints a plan and writes `report.json` without mutating skills or memory.
- [x] Apply path creates `pre-migration-backup.zip` unless `--no-backup`, copies/archives planned items, then runs skill import.
- [x] EN/ZH README list the new commands.
- [x] Unit tests cover env allowlist filtering and markdown planning.

## Risks

- Risk: Users expect full OpenClaw parity including sessions and channels.
  - Impact: Support confusion.
  - Mitigation: Archive unmapped config; document MVP scope in README and report notes.

## Validation Plan

- Required tests: `cargo test -p skilllite-commands migrate`
- Commands to run: `cargo run -p skilllite -- claw migrate --help`; fixture `--dry-run`
- Manual checks: inspect `report.json` and backup zip on apply

## Regression Scope

- Areas likely affected: `skilllite-commands` skill import re-exports; CLI dispatch.
- Explicit non-goals: Changing `import-openclaw-skills` behavior.

## Links

- Related docs: README.md, docs/zh/README.md
