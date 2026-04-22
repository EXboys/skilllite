# PRD

## Background

`TASK-2026-046` added local ZIP support to `skilllite add`, which means the
desktop bridge can already install ZIP skills if given a file path. The current
desktop UI still exposes only a generic source text field, so the capability is
hard to discover and inconvenient for typical desktop users.

## Objective

Make ZIP skill import discoverable in the desktop UI by adding a native file
picker button that feeds the selected path into the existing add flow.

## Functional Requirements

- FR-1: The skill panel must include a button that opens a native file picker
  restricted to ZIP-like files.
- FR-2: After a user selects a ZIP file, the desktop must invoke the existing
  `skilllite_add_skill` pathway with that local path and show the same success /
  error result area as the manual add flow.
- FR-3: Cancelling the picker must not change current add state or show a false error.

## Non-Functional Requirements

- Security: The picker must not bypass the existing backend install path or add
  any new privileged filesystem access beyond user-selected files.
- Performance: The button should not introduce extra work until the user
  explicitly opens the picker.
- Compatibility: Must work with Tauri's dialog plugin on supported desktop
  platforms and preserve the existing repo/source text input flow.

## Constraints

- Technical: Reuse the current `skilllite_add_skill` Tauri command; no new Rust
  command surface for this step.
- Timeline: Keep the change UI-scoped and small so it can ship immediately after
  the backend ZIP support.

## Success Metrics

- Metric: Desktop users can install a ZIP package without manually typing a path.
- Baseline: ZIP install is only available by pasting a local path into the source input.
- Target: A visible button opens a picker and routes the selected ZIP through the add flow.

## Rollout

- Rollout plan: Ship as a minor desktop UI enhancement on top of the existing
  ZIP-capable add flow.
- Rollback plan: Remove the picker button and copy changes; leave backend ZIP support intact.
