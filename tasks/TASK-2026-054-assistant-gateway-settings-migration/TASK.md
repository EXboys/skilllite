# TASK Card

## Metadata

- Task ID: `TASK-2026-054`
- Title: Assistant persisted settings migration for gateway serve
- Status: `done`
- Priority: `P2`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

The Assistant code now uses gateway-oriented naming, but it still keeps runtime fallbacks to legacy persisted `channelServe*` fields. That compatibility path is useful temporarily, but it leaves the store shape and runtime reads more complicated than necessary.

This task adds a versioned persisted-settings migration so existing local data is moved forward mechanically, after which runtime code can read only the new `gatewayServe*` fields.

## Scope

- In scope:
  - Add a versioned Zustand persist migration for Assistant settings.
  - Migrate legacy `channelServeBind` / `channelServeToken` values into `gatewayServeBind` / `gatewayServeToken`.
  - Remove runtime fallback reads of legacy `channelServe*` fields from the gateway settings component.
  - Stop storing legacy `channelServe*` defaults in the active settings shape.
- Out of scope:
  - Removing or migrating any unrelated persisted settings fields.
  - Adding a broader persistence test harness.
  - Gateway backend changes.

## Acceptance Criteria

- [x] Assistant persisted settings have an explicit version and migration path.
- [x] Existing users with only `channelServe*` local values are migrated to `gatewayServe*` on load.
- [x] Runtime gateway settings UI reads only the new gateway fields.
- [x] Assistant build and repo validation pass.

## Risks

- Risk:
  - Persist migration could accidentally preserve legacy keys and keep re-writing them.
  - Mitigation:
    - Strip `channelServe*` during migration and return only the new settings shape.

## Validation Plan

- Required tests:
  - Assistant frontend production build.
  - Task document validation.
- Commands to run:
  - `npm run build`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm gateway settings UI reads only `gatewayServe*` values in runtime code.

## Regression Scope

- Areas likely affected:
- Explicit non-goals:

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
