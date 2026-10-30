# Status Journal

## Timeline

- 2026-10-30:
  - Progress: Reproduced `EAI_AGAIN` (`-3`) for `example.com` inside the unproxied agent bwrap mount set. Mounting `/etc/resolv.conf` made `getaddrinfo` succeed. Implemented read-only binds for `/etc/resolv.conf`, `/etc/nsswitch.conf`, and `/etc/hosts` only when no proxy is set. Proxied plans still omit them.
  - Blockers: None.
  - Next step: None.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated

## Validation evidence

- Manual bwrap (bubblewrap 0.9.0), same mounts as the old agent plan, no resolv.conf: `LOOKUP_FAIL gaierror [Errno -3] Temporary failure in name resolution`.
- Same plan plus the three name-service files: `lookup ('172.66.147.243', 443)`.
- Numeric `127.0.0.1` resolved without those files, which is why proxied runs (IP in `env_url`) did not show the bug.
- Falsify: with `push_name_service` removed, `linux_run_without_proxy_mounts_name_service_files` failed: `missing /etc/resolv.conf in [] stderr=`.
- Restored mounts: that test passed.
- `cargo fmt --check` exit 0.
- `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` exit 0.
- `cargo test -p skilllite-sandbox` exit 0 (`running 115 tests`, process exit 0).
- `cargo test -p skilllite --test e2e_minimal` : `2 passed; 0 failed`.
- `cargo test -q` workspace exit 0. No `FAILED` lines.
- `cargo clippy --all-targets -- -D warnings` exit 0 (`Finished dev profile`).
