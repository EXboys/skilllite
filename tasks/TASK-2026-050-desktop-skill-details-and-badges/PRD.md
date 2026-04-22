# PRD

## Background

The desktop app now shows all installed skills, including non-script packages.
However, users still lack fast context about what each skill is, whether it can
run directly, where it came from, and what setup is still missing after install.

## Objective

Make the desktop skills panel self-explanatory by surfacing skill type, source,
trust, dependencies, and missing setup requirements directly in the UI.

## Functional Requirements

- FR-1: The skill list must display a type badge per skill.
- FR-2: The panel must show source / trust / dependency details for the selected skill.
- FR-3: Successful add flows must include missing command / env setup hints when applicable.

## Non-Functional Requirements

- Security: Reuse existing manifest / metadata / dependency parsing without adding
  new network or filesystem mutation paths.
- Performance: Skill detail derivation should remain acceptable for normal desktop
  skill counts.
- Compatibility: Existing script-backed skills continue to render; non-script
  skills gain richer presentation.

## Constraints

- Technical: Build the desktop DTO from existing `skilllite-core` metadata and
  manifest helpers rather than inventing a separate parsing standard.
- Timeline: Implement as a bounded UX iteration on top of the recent desktop skill list fixes.

## Success Metrics

- Metric: Users can identify skill type and missing setup directly from the panel.
- Baseline: Skill rows show only the name and folder-open action.
- Target: Skill rows and the selected-skill detail area show meaningful metadata,
  and add-result warnings highlight missing setup immediately.

## Rollout

- Rollout plan: Ship as a desktop-only UI/bridge enhancement.
- Rollback plan: Revert DTO enrichment and return to the simple name-only list.
