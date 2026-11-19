# Status Journal

## Timeline

- 2026-11-19:
  - Progress: Unfixed `skilllite-sandbox run .skills/calculator` exited 1 with `bwrap: Can't read seccomp data: Bad file descriptor`. After skipping `close` when the BPF file is already descriptor 3, the same command printed `{"operation": "add", "a": 1.0, "b": 2.0, "result": 3.0}` at sandbox level 3 and level 2 (exit 0). `cargo test -p skilllite-sandbox` passed (116 lib tests). Closing descriptor 3 again made `seccomp_fd_stays_open_when_the_filter_is_already_fd_3` fail with code 2.
  - Blockers: none.
  - Next step: none.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
