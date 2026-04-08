# PRD

## Goal

Provide an optional HTTP API and OpenAPI specification so any language can interoperate with SkillLite run-scoped artifact storage without a native SDK.

## User-visible behavior

- Not shipped in the default CLI; library-only integration.
- When embedded, clients use documented URLs and optional `Authorization: Bearer`.

## Non-goals

CLI subcommand to start the server, Python SDK wiring in this task.
