# PRD

## What

Add OpenClaw migration MVP: `skilllite claw migrate` and `skilllite migrate openclaw`.

## Why

Reduce switching cost from OpenClaw-style agent homes without promising full platform parity.

## Decisions

- Reuse `import-openclaw-skills` for skill copy, scan, manifest, and deps.
- Persona lands in workspace `.skilllite/SOUL.md`; memory Markdown under `chat_root()/memory/`.
- Secrets merge only from an allowlisted key set when `--migrate-secrets` is set.
- Unmapped OpenClaw JSON and workspace instruction files copy into the migration report `archive/` subtree.

## Non-goals

- Hermes `hermes migrate` reverse path.
- Session transcript import.
- Live channel or TTS configuration migration.
