# TASK Card

## Metadata

- Task ID: `TASK-2026-052`
- Title: Assistant settings migrate from channel serve to gateway serve
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

Assistant still guides users to `skilllite channel serve` even though the repository now has a preferred unified host: `skilllite gateway serve`. This mismatch leaves the desktop UI on an outdated operating model and hides the new artifact-hosting capability from the settings surface.

The migration needs to keep the current UX shape (copy command, show URLs, run local health check) while updating wording, stored settings, and generated commands to the new gateway host.

## Scope

- In scope:
  - Migrate the settings panel copy/start command from `skilllite channel serve` to `skilllite gateway serve`.
  - Add optional `artifact-dir` support in the Assistant settings UI and persisted settings.
  - Update labels/help text/i18n to describe gateway as the preferred host while keeping the page under the same settings tab.
  - Keep local `/health` probing working for the gateway host.
  - Update Assistant README/user-facing docs in scope of this UI change.
- Out of scope:
  - Auto-starting the gateway process from Assistant.
  - Renaming the settings tab id or changing broader settings navigation architecture.
  - Migrating other SDK/UI surfaces to gateway terminology outside Assistant.
  - Implementing websocket/session-routing controls in the settings page.

## Acceptance Criteria

- [x] The Assistant settings panel generates `skilllite gateway serve` commands instead of `skilllite channel serve`.
- [x] Users can optionally configure an artifact directory and see the artifact URL when provided.
- [x] Local health checks still work against the configured gateway `/health` URL.
- [x] Persisted Assistant settings include gateway host fields without breaking existing local data.
- [x] Relevant Assistant docs/i18n are updated and validation passes.

## Risks

- Risk: Existing local users lose their saved bind/token values.
  - Impact: Settings migration would feel destructive.
  - Mitigation: Reuse the existing stored values as compatibility fallbacks and persist into new gateway-specific keys.
- Risk: The UI implies Assistant now manages the process lifecycle.
  - Impact: Users expect one-click start/stop that does not exist.
  - Mitigation: Keep explicit wording that the desktop app does not auto-start the listener.
- Risk: Artifact configuration confuses users who only need webhook ingress.
  - Impact: Added complexity on a previously simple page.
  - Mitigation: Keep `artifact-dir` optional and explain that artifact routes are mounted only when set.

## Validation Plan

- Required tests:
  - `python3 scripts/validate_tasks.py`
  - `npm run build` in `crates/skilllite-assistant`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `npm run build`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- Manual checks:
  - Re-read the settings component and stored keys to confirm gateway terminology is consistent.
  - Re-read README/i18n entries for EN/ZH consistency.
  - Re-read `tasks/board.md` after status updates.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-assistant/src/components/ChannelServeSettingsSection.tsx`
  - `crates/skilllite-assistant/src/stores/useSettingsStore.ts`
  - `crates/skilllite-assistant/src/i18n/messages/en.ts`
  - `crates/skilllite-assistant/src/i18n/messages/zh.ts`
  - `crates/skilllite-assistant/README.md`
  - `tasks/TASK-2026-052-assistant-gateway-settings/*`
- Explicit non-goals:
  - No backend gateway behavior changes.
  - No Assistant process-management feature.
  - No broad settings navigation rename beyond text copy.

## Links

- Source TODO section:
  - Follow-up to `TASK-2026-051-gateway-phase1-bootstrap`.
- Related PRs/issues:
- Related docs:
  - `crates/skilllite-assistant/README.md`
  - `docs/en/ENV_REFERENCE.md`
  - `docs/zh/ENV_REFERENCE.md`
