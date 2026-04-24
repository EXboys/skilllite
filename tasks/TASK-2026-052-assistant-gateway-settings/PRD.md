# PRD

## Background

The repository now treats `skilllite gateway serve` as the preferred unified HTTP host for inbound webhook and optional artifact routes. However, the Assistant settings page still presents the old `channel serve` model. This leaves the desktop product on an outdated mental model and prevents users from discovering the unified host path.

## Objective

Assistant users can configure and copy a valid `skilllite gateway serve` command directly from settings, including optional artifact hosting, without losing the current health-check UX.

## Functional Requirements

- FR-1: The settings panel displays gateway-focused wording and builds a `skilllite gateway serve` command.
- FR-2: The panel supports `bind`, `token`, and optional `artifact-dir`.
- FR-3: The panel shows `/health` and `/webhook/inbound` URLs for all users, plus artifact API URL when `artifact-dir` is set.
- FR-4: Existing health checking continues to probe the configured loopback `/health`.
- FR-5: Existing saved `channelServe*` local settings are migrated or reused so users do not lose current values.

## Non-Functional Requirements

- Security:
  - The UI must continue to emphasize that the desktop app does not automatically start the listener.
  - Token handling remains local-only and clearly marked as sensitive.
- Performance:
  - No new background polling or heavy runtime work should be introduced.
- Compatibility:
  - Existing local persisted settings should continue to populate the new page.
  - The migration should not require users to manually re-enter bind/token.

## Constraints

- Technical:
  - Reuse the existing Tauri health probe rather than inventing a new backend command.
  - Keep the tab/component surface stable enough to avoid unnecessary broader settings refactors.
- Timeline:
  - One task only: settings migration and doc copy, no process-control features.

## Success Metrics

- Metric: Settings-generated host command.
- Baseline: `skilllite channel serve`
- Target: `skilllite gateway serve`
- Metric: User-visible host capabilities.
- Baseline: webhook-only in Assistant UI
- Target: unified host with optional artifact-dir surfaced in Assistant UI

## Rollout

- Rollout plan:
  - Ship the Assistant UI change after the gateway host exists.
  - Keep existing backend compatibility fallbacks in persisted settings.
- Rollback plan:
  - If the UI proves confusing, revert wording/fields while keeping the backend gateway intact.
