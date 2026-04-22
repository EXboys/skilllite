# Status Journal

## Timeline

- 2026-04-22:
  - Progress: Confirmed the desktop list does refresh after add, but its bridge
    only surfaces script-backed skills; root cause reproduced with installed
    `web-search` present in manifest and on disk but absent from the desktop list.
  - Blockers: None.
  - Next step: Switch desktop discovery to all installed skill instances and add regression coverage.
- 2026-04-22:
  - Progress: Switched desktop discovery from script-only filtering to all discovered
    skill instances, which also fixed open-directory and remove resolution for
    non-script skills; added regression coverage and updated EN/ZH desktop docs.
  - Blockers: Manual desktop smoke-check was not run in this session.
  - Next step: Task complete.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
