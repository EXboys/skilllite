# PRD

## Background

SkillLite evolution data is now project-scoped for desktop and CLI L2 flows. `status`, `backlog`, `pending`, `confirm`, `reject`, and `run` accept or derive a workspace and use project-local `chat/` plus the effective skills root (`skills/` with `.skills` fallback).

The remaining maintenance commands `reset` and `repair-skills` did not move with that contract. A destructive reset can therefore modify the wrong chat store while leaving current evolved skills in place, and repair can silently validate an empty legacy tree.

## Objective

Make evolution reset and repair target the same workspace roots as the rest of the evolution command surface.

Prevent split-brain behavior where chat state and evolved skills are reset or validated in different workspaces.

## Functional Requirements

- FR-1: `evolution reset` MUST accept `--workspace/-w` with default `"."` and use `<workspace>/chat` for prompt/log/snapshot reset.
- FR-2: `evolution reset --force` MUST remove `_evolved` under the effective skills root for the selected workspace.
- FR-3: `evolution repair-skills` MUST accept `--workspace/-w` with default `"."` and validate the effective skills root for that workspace.
- FR-4: Desktop repair calls MUST pass the resolved workspace explicitly to the CLI subprocess.

## Non-Functional Requirements

- Security: No sandbox, permission, or LLM credential handling changes.
- Performance: Root resolution remains filesystem-only and should not add meaningful overhead.
- Compatibility: Legacy `.skills` workspaces continue to work through existing fallback behavior; explicit workspace selection is available for non-current workspaces.

## Constraints

- Technical: Keep dependency direction unchanged (`skilllite` entry dispatches into `skilllite-commands`).
- Timeline: N/A for autonomous execution; this is a focused CLI bugfix.

## Success Metrics

- Metric: Regression tests cover reset root selection and repair root selection.
- Baseline: Reset/repair use legacy `.skills` or global chat roots in at least one common workspace scenario.
- Target: Tests fail on the old behavior and pass with workspace-aware resolution.

## Rollout

- Rollout plan: Ship as a CLI/desktop bugfix with task evidence and focused tests.
- Rollback plan: Revert the CLI argument and resolver changes if unexpected compatibility issues are found.
