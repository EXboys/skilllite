# Status Journal

## Timeline

- 2026-11-20:
  - Progress: Confirmed `wait_with_timeout` treats `RUSAGE_CHILDREN` `ru_maxrss` as the current child's peak. Before the fix, `later_command_keeps_stdout_when_an_earlier_child_peaked_higher` failed with `Process rejected: peak memory (57 MB) exceeded limit (4 MB)` and exit code `-1`. The pre-reap comparison keeps that stdout and still rejects a child that raises the mark.
  - Blockers: None.
  - Next step: None.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
