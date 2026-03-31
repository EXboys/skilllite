# Architecture Boundaries (Hard Rules)

Scope: any task that changes Rust workspace layout, crate dependencies, or entry-layer routing.

## Must

- Keep dependency direction strictly one-way:
  `entry (skilllite CLI/MCP/stdio) -> commands -> agent -> executor -> sandbox -> core`.
- Keep `core` as a pure lower-layer capability crate with no upper-layer dependencies.
- Add new capabilities via extension registration points (for example, `extensions/registry`), not hardcoded branches in the main loop.
- Use explicit interface types (struct/trait) for cross-layer calls instead of coupling to upper-layer concrete implementations.
- When adding a crate or feature, sync dependency and boundary docs in both:
  `docs/en/ARCHITECTURE.md` and `docs/zh/ARCHITECTURE.md`.

## Must Not

- Do not make `core` depend on `agent`, `commands`, `executor`, or entry-layer `skilllite`.
- Do not leak platform-specific implementation details (macOS/Linux/Windows) into upper business crates.
- Do not add new tools in `agent_loop` via `if tool_name == "xxx"` branching.
- Do not break crate boundaries as a "temporary fix."

## Change Checklist

- [ ] Did this change alter crate dependency direction?
- [ ] Did this introduce direct cross-layer coupling instead of interface-based coupling?
- [ ] Did all new capability wiring go through extension registration?
- [ ] Were architecture docs updated in both EN and ZH?

## Quick Verify

- `cargo check --workspace`
- `cargo clippy --all-targets`
- `cargo test`
