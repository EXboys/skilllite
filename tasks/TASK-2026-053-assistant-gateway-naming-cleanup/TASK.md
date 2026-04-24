# TASK Card

## Metadata

- Task ID: `TASK-2026-053`
- Title: Assistant gateway naming cleanup
- Status: `done`
- Priority: `P2`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

After migrating the Assistant settings page to `gateway serve`, several code-facing names still reference `channelServe` or `assistant_channel_health_probe`. This leaves the implementation with mixed terminology that is harder to maintain and makes future gateway-focused work more confusing than necessary.

## Scope

- In scope:
  - Rename the Assistant settings component/symbol usage from channel-oriented naming to gateway-oriented naming.
  - Rename i18n keys from `settings.channelServe.*` to `settings.gatewayServe.*`.
  - Rename the Tauri health-probe command and related helper names from channel-oriented naming to gateway-oriented naming.
  - Preserve local-settings compatibility fields that still need to read old persisted data.
- Out of scope:
  - Removing legacy persisted `channelServe*` settings fields.
  - Broader Assistant settings navigation refactors.
  - Gateway backend behavior changes.

## Acceptance Criteria

- [x] Assistant component/import names are gateway-oriented rather than channel-oriented.
- [x] Assistant i18n keys use `settings.gatewayServe.*` consistently.
- [x] Tauri health probe command naming is updated to gateway terminology and frontend calls still work.
- [x] Legacy persisted settings compatibility is preserved.
- [x] Validation passes.

## Risks

- Risk:
  - Impact:
  - Mitigation:

## Validation Plan

- Required tests:
- Commands to run:
- Manual checks:

## Regression Scope

- Areas likely affected:
- Explicit non-goals:

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
