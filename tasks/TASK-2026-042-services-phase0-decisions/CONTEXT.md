# Technical Context

## Current State

- Relevant crates/files:
  - `Cargo.toml` (root workspace; currently `exclude = ["crates/skilllite-assistant", "crates/crates"]`).
  - `crates/skilllite-assistant/src-tauri/Cargo.toml` (Desktop manifest; depends on `skilllite-core`, `skilllite-evolution`, `skilllite-sandbox`, `skilllite-agent`, `skilllite-fs`).
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/**` (Desktop bridge directly calls `skilllite_sandbox::*`, `skilllite_evolution::*`, `skilllite_agent::*`, `skilllite_core::*`).
  - `crates/skilllite-commands/src/**` (CLI command implementations; sync `pub fn`, `anyhow::Result`).
  - `deny.toml` (wrapper allow-list for workspace crates; currently does not cover Desktop manifest).
  - `docs/en/ENTRYPOINTS-AND-DOMAINS.md`, `docs/zh/ENTRYPOINTS-AND-DOMAINS.md` (still describes Desktop primarily as "GUI over installed `skilllite` binary").
  - `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md` (says Desktop "build time: skilllite-core" only).

- Current behavior:
  - Desktop builds via a separate Tauri manifest (not part of root workspace members).
  - Desktop bridge bypasses `skilllite-commands` and reaches into multiple domain crates directly.
  - Some shared use cases (workspace discovery, runtime probe/provision, evolution status/diagnostics) are implemented twice: once in `skilllite-commands` for CLI, once in `skilllite_bridge` for Desktop.
  - No explicit shared application service layer exists today.

## Architecture Fit

- Layer boundaries involved:
  - Current declared chain (`spec/architecture-boundaries.md`): `entry → commands → agent → executor → sandbox → core`.
  - Reality: Desktop entry currently couples to `core / fs / sandbox / executor / agent / evolution` directly, sidestepping `commands`.
- Interfaces to preserve:
  - All existing CLI subcommands and their stdout/stderr contract.
  - All existing Tauri commands exposed by `skilllite_bridge`.
  - All existing MCP tool schemas.
  - All existing Python SDK subprocess/IPC behavior.

## Dependency and Compatibility

- New dependencies:
  - None in this TASK (no source code changes outside `deny.toml`, CI config, and docs).
  - Future Phase 1A bootstrap TASK will add a new path crate `skilllite-services` (initially empty).
- Backward compatibility notes:
  - This TASK does not change runtime behavior; existing users see no observable change.
  - `deny.toml` extension to Desktop manifest may surface previously hidden allow-list gaps; those must be fixed before merge.

## Design Decisions

- Decision D1 — Desktop is a first-class entry.
  - Rationale: Desktop already directly consumes multiple core crates; treating it as "shell" misrepresents reality and prevents any meaningful boundary enforcement.
  - Alternatives considered: Continue treating Desktop as non-core product and reduce its direct coupling to domain crates.
  - Why rejected: Reverting Desktop to a thin shell would require large UX/feature regression and is not aligned with current product direction.

- Decision D2 — New crate `skilllite-services` for shared application services.
  - Rationale: Provides a clean entry-neutral home; preserves single-responsibility for `skilllite-commands` (CLI presentation) and `skilllite-agent` (agent orchestration).
  - Alternatives considered:
    - A) Hosting shared services inside `skilllite-commands`.
      - Why rejected: `skilllite-commands` is CLI-shaped (sync, `anyhow`, terminal-flavored APIs); Desktop depending on it would re-introduce the same form mismatch we are trying to remove.
    - B) Hosting shared services inside `skilllite-agent`.
      - Why rejected: `skilllite-agent` is the agent-loop crate; widening its role would break single-responsibility and inflate compile graphs.

- Decision D3 — Async by default; per-crate `thiserror`.
  - Rationale: Desktop bridge and `skilllite-agent` are already async; keeping services sync would force blocking wrappers in two of the three primary callers. `thiserror` keeps `spec/rust-conventions.md` compliance and lets adapters convert errors at the boundary.
  - Alternatives considered:
    - Default-sync with `block_on` adapters at async sites.
      - Why rejected: Would invert the cost; async callers are the majority.
    - Single repo-wide `anyhow::Error` for service results.
      - Why rejected: Violates per-crate typed-error rule in `spec/rust-conventions.md`.

- Decision D4 — `cargo deny check bans` covers Desktop manifest.
  - Rationale: D1 admits Desktop is first-class; without deny coverage, the architectural boundary rule becomes unenforceable on the Desktop side.
  - Alternatives considered: PR-review checklist only.
  - Why rejected: Manual review does not provide the mechanical evidence required by `spec/verification-integrity.md`.

- Decision D5 — Serde-serializable, platform-neutral service interfaces; no cross-language binding promised this round.
  - Rationale: Keeps the door open for future MCP / Python SDK reuse without committing the design surface now.
  - Alternatives considered: Expose Tauri / `tokio::sync` types directly.
  - Why rejected: Would tightly couple service interfaces to Desktop-only details.

## Open Questions

- [ ] Exact CI workflow file to extend with the Desktop `cargo deny` invocation (likely `.github/workflows/ci.yml`).
- [ ] Whether `deny.toml` should split a separate `[bans]` section for Desktop manifest, or rely on the current single section with extended wrappers.
- [ ] Whether Phase 1A bootstrap TASK should land the empty `skilllite-services` crate immediately after this TASK, or wait for an explicit go signal.
