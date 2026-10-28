# Status Journal

## Timeline

- 2026-10-28:
  - Progress: nftables 1.0.9 rejects a leaf cgroup name (`cgroupv2 path fails: No such file or directory`). The same rule with the path relative to `/sys/fs/cgroup` and `level` equal to the component count installs. From inside that cgroup, the proxy port connects and a different local TCP port times out. `proxy_nft_script` now emits that path. Sandbox tests, clippy, and `e2e_minimal` passed.
  - Blockers: none.
  - Next step: none.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
