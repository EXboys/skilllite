# PRD

## Background

P4 in-tree extract landed on `main` (#158). The user wants a **separate GitHub project** for the desktop GUI.

## Objective

- Desktop lives at `https://github.com/EXboys/skilllite-assistant` (once the empty repo is created and pushed).
- `EXboys/skilllite` is engine-only and links to that project.

## Functional Requirements

- FR-1: Standalone repo root has README, LICENSE, npm/Tauri, and CI.
- FR-2: Engine repo no longer ships the Tauri/React source tree.
- FR-3: User-facing EN/ZH docs name the new GitHub project.

## Non-Functional Requirements

- Security: Desktop remains subprocess-only over the `skilllite` binary.
- Compatibility: Installers can move to the new repo's Releases after cutover.

## Constraints

- Technical: Cloud token cannot `createRepository` on `EXboys`.
- Timeline: N/A.

## Success Metrics

- Metric: Engine README and START_PATHS link to the assistant GitHub URL
- Target: No Tauri app sources under `skilllite-assistant/` in the engine repo except a stub

## Rollout

- Rollout plan: Push export branch after the owner creates an empty public repo; merge engine pointer PR.
- Rollback plan: Restore `skilllite-assistant/` from the export branch.
