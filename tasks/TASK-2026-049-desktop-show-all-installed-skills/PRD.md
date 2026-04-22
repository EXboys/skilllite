# PRD

## Background

Desktop users can now install ZIP-packaged skills, including bash-tool and other
non-script skills. The current desktop discovery path only lists script-backed
skills, so successfully installed packages can disappear from the visible skill
list even though they exist on disk and in the manifest.

## Objective

Make the desktop skills panel represent all installed/discovered skills in the
workspace, not only those with script files.

## Functional Requirements

- FR-1: The desktop skill list must include any discovered skill directory with
  `SKILL.md`, regardless of whether it contains scripts.
- FR-2: Desktop open-folder and delete actions must resolve those same skills.

## Non-Functional Requirements

- Security: No relaxation of install-time admission or runtime execution rules.
- Performance: Keep discovery cost equivalent to current workspace scan complexity.
- Compatibility: Existing script-backed skills must continue to appear and behave
  as before.

## Constraints

- Technical: Reuse `skilllite_core::skill::discovery::discover_skill_instances_in_workspace`
  instead of duplicating another discovery algorithm.
- Timeline: Small desktop-bridge correction following ZIP import enablement.

## Success Metrics

- Metric: Installed non-script skills appear in the desktop list.
- Baseline: Non-script skills are installed but filtered out from the desktop view.
- Target: Installed non-script skills are visible and manageable from the desktop panel.

## Rollout

- Rollout plan: Ship as a desktop bridge fix with no CLI changes.
- Rollback plan: Restore script-only filtering if unexpected UI regressions appear.
