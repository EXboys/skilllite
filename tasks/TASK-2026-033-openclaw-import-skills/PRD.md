# PRD

## What

Add `skilllite import-openclaw-skills` to aggregate skills from OpenClaw-style directories into the project skills root, with Hermes-like source precedence and conflict handling.

## Why

Lower migration cost from OpenClaw without implementing full `migrate` parity.

## Decisions

- Reuse `skilllite add` admission scan, filtered copy, manifest upsert, and dependency install.
- Source order: per-workspace `skills` then `.agents/skills` for each workspace candidate; then `~/.openclaw/skills`; legacy bot homes; then `~/.agents/skills`.
- Manifest `source` prefix: `openclaw-import:<tag>`.

## Non-goals

- Importing SOUL/MEMORY/JSON config from OpenClaw.
