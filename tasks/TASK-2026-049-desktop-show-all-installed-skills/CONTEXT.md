# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
  - `crates/skilllite-core/src/skill/discovery.rs`
- Current behavior:
  - Desktop list/open/delete routes currently go through `discover_scripted_skill_instances()`.
  - That helper filters discovered skills with `skill_has_scripts()`, excluding
    bash-tool and prompt-only skills.
  - Core discovery already exposes all `SkillInstance` values with `SKILL.md`.

## Architecture Fit

- Layer boundaries involved:
  - Desktop bridge only; no CLI/protocol boundary change.
- Interfaces to preserve:
  - `list_skill_names(workspace)`
  - `find_skill_dir(workspace, skill_name)`
  - desktop Tauri commands already calling those helpers.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Script-backed skills remain visible.
  - Non-script skills become newly visible/manageable.

## Design Decisions

- Decision: Replace the script-only helper with all-skill discovery for desktop list/open/delete.
  - Rationale: Those desktop actions represent installed skills, not just executable scripts.
  - Alternatives considered: Keep list script-only and add a second "other skills" section.
  - Why rejected: More UI complexity for a discovery bug.

## Open Questions

- [ ] Should the desktop UI eventually render different badges for script-backed vs bash-tool vs prompt-only skills?
- [ ] Should repair actions later skip non-script skills explicitly in the UI?
