# Status Journal

## Timeline

- 2026-10-28:
  - Progress: Task opened. Implementation of `skilllite_sandbox::run` started.
  - Blockers: none.
  - Next step: tests and docs.
- 2026-10-28:
  - Progress: `skilllite_sandbox::run` and `skilllite agent-sandbox` are implemented. macOS Seatbelt uses `localhost:<port>` for a loopback proxy and rejects other addresses. Linux nftables uses the cgroup depth under `/sys/fs/cgroup`.
  - Blockers: none. Linux `bwrap` and nftables were not executed on this darwin host.
  - Next step: none in this repository. Pool, binding, and SSE stay in the private repository.
- 2026-10-28:
  - Progress: missing-proxy refusal is now `require_proxy` (CLI `--require-proxy`, env `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY`). Default is off. A provided proxy is still enforced.
  - Blockers: none.
  - Next step: none.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
