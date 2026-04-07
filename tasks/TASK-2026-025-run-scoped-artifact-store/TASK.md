# TASK Card

## Metadata

- Task ID: `TASK-2026-025`
- Title: Run-scoped artifact store (trait, local impl, injection)
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-07`
- Target milestone:

## Problem

Multi-step **super agent** and **skill** workflows need a **run-scoped** way to pass data between steps. Local, low-volume work (e.g. reports, slides) tolerates ad-hoc files via existing `write_file`/`read_file` tools; **production** integrations across many systems produce **high-volume, multi-hop data flow** and become unreliable without a **first-class, pluggable storage mechanism**.

Note: For **LLM-mediated** cross-step data passing (small/medium payloads), the existing `write_file`/`read_file` tools already work. The value of this task is **not** adding new LLM-callable tools, but establishing the **architectural contract** (trait + pluggable backend + run scoping) that enables: (a) production environments to swap in enterprise storage behind the same interface, and (b) a future v1 where **skill subprocess code** can programmatically read/write artifacts via env var or SDK — which is the real production bottleneck.

Users expect:

1. A **standard framework interface** for run-scoped artifact read/write.
2. A **built-in default store** so basic flows work without external infrastructure.
3. The ability to **swap in their own storage** (object store, internal APIs, DB) behind the same interface.

Rust should remain the **source of truth** for types and default behavior; other languages may bind later (e.g. PyO3 or HTTP).

## Scope

- In scope:
  - Define `ArtifactStore` trait and structured `StoreError` (or equivalent) in `skilllite-core`, with clear `run_id` + logical `key` semantics.
  - Implement `LocalDirArtifactStore` in `skilllite-agent` (implements core trait; safe key handling, atomic writes under a configurable base directory).
  - Wire **injection** into agent run context (`ChatSession` — not `AgentConfig`, which is `Clone + Debug` and cannot hold `dyn` trait objects): each run has a `run_id` and a store handle usable from tool/skill execution paths.
  - Unit tests for the local store and error cases; follow `spec/testing-policy.md` for the touched crates.
  - If crate boundaries or dependency direction change: update `docs/en/ARCHITECTURE.md` and `docs/zh/ARCHITECTURE.md` per `spec/architecture-boundaries.md` and `spec/docs-sync.md`.
- Out of scope (explicit deferrals):
  - Shipping a cloud reference implementation (S3, etc.) in-tree.
  - Python package / PyO3 bindings (track separately).
  - Streaming API, TTL, listing, or large-object handles beyond minimal v0 unless pulled in by acceptance needs.
  - **Executor/sandbox** wiring (e.g. `SKILLLITE_ARTIFACTS_DIR` for subprocess skills) as a **required** part of v0—this is the **v1 priority** that unlocks production skill-code-level artifact access, once the agent-side contract is stable.
  - New LLM-callable tools (`artifact_put`/`artifact_get`) — existing `write_file`/`read_file` already handle LLM-mediated cross-step data; adding dedicated tools would overlap without clear benefit in v0.

## Acceptance Criteria

- [ ] `skilllite-core` exposes an `ArtifactStore` abstraction and structured errors; semantics documented (run scope, key rules, versioning note if any).
- [ ] `skilllite-agent` provides `LocalDirArtifactStore` implementing that trait without violating dependency direction (`executor`/`sandbox`/`core` do not depend on `agent`).
- [ ] `ChatSession` (or an equivalent runtime context — **not** `AgentConfig` which must remain `Clone + Debug`) carries an injectable store (`Arc<dyn ArtifactStore + Send + Sync>`) tied to the active `run_id`. In **run mode**, `run_id` aligns with `RunCheckpoint.run_id`; in **chat mode**, v0 may either use `session_key` as scope or explicitly not support artifacts (decision recorded in `CONTEXT.md`).
- [ ] At least one **documented integration point** exists showing how future tools or v1 subprocess wiring would consume the store. The propagation path from `ChatSession` → `agent_loop` must be defined (e.g. via shared context object or explicit parameter). v0 does **not** require new LLM-callable tools — existing `write_file`/`read_file` remain the LLM-level mechanism for cross-step data.
- [ ] Tests pass for affected crates; `cargo clippy --all-targets` clean for changed code per `spec/rust-conventions.md`.
- [ ] If architecture docs are required by boundary changes, EN and ZH stay in sync.

## Risks

- Risk: Wrong dependency direction (e.g. `core` or `executor` depending on `agent` for the local store type).
  - Impact: Violates `spec/architecture-boundaries.md`; hard to reuse from other entrypoints.
  - Mitigation: Keep trait in `core`; concrete `LocalDirArtifactStore` only in `agent`; pass paths into `executor` when needed instead of importing agent types from lower layers.
- Risk: Key/path traversal or unsafe filenames in `LocalDirArtifactStore`.
  - Impact: Security and data integrity issues in sandbox-adjacent flows.
  - Mitigation: Reuse or align with `skilllite-fs` validation patterns; tests for `..` and oversize keys.
- Risk: Scope creep (streaming, multi-tenant quotas, HTTP service).
  - Impact: Delays v0 that unblocks production adapters.
  - Mitigation: Ship minimal `get`/`put` + errors + injection; defer advanced features to follow-up tasks.
- Risk: `AgentConfig` is `#[derive(Debug, Clone)]`; adding `Arc<dyn ArtifactStore>` would break both derives.
  - Impact: Compile failure or forced removal of `Clone`/`Debug` across all call sites.
  - Mitigation: Inject store into `ChatSession` (runtime context) instead of `AgentConfig` (value-type configuration).
- Risk: Sync vs async trait mismatch — production backends (S3, HTTP) are inherently async; `LocalDirStore` is sync.
  - Impact: If the trait is sync-only, async backends must `block_in_place`; if async-only, the local impl needs an async runtime for trivial I/O.
  - Mitigation: v0 uses **sync trait** (matches existing `run_checkpoint` pattern); callers in async context use `spawn_blocking`. Document migration path to async trait for v1 if needed.

## Validation Plan

- Required tests: unit tests for `LocalDirArtifactStore` (happy path, missing key, invalid key, overwrite behavior if defined).
- Commands to run: `cargo test -p skilllite-core`, `cargo test -p skilllite-agent` (and any crate touched); `cargo check --workspace`; `cargo clippy --all-targets`.
- Manual checks: one local multi-step scenario (or scripted) verifying artifacts persist under the expected run directory.

## Regression Scope

- Areas likely affected: `skilllite-core`, `skilllite-agent`, possibly `skilllite-commands` / CLI config if store is configurable; architecture docs.
- Explicit non-goals: changing sandbox security policy beyond passing a path; adding new env vars without `docs-sync` (if env vars are introduced, document EN/ZH).

## Links

- Spec routing (implementation): `spec/README.md` — task type **mixed** (agent behavior + architecture boundaries + Rust); inject `verification-integrity.md`, `task-artifact-language.md`, `architecture-boundaries.md`, `structured-signal-first.md`, `rust-conventions.md`, `testing-policy.md`, `docs-sync.md` when implementing.
- Source discussion: user feedback on local vs production data flow; design chat 2026-04-07 (artifact interface + default local store + pluggable backends).
- Related docs: `spec/architecture-boundaries.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.
- Related PRs/issues: (add when opened)
