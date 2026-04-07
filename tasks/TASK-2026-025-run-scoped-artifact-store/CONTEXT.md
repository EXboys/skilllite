# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-core` (config, shared types), `crates/skilllite-agent` (agent loop, `AgentConfig` in `types/config.rs`, `ChatSession` in `chat_session.rs`, checkpoint / `run_id` in `run_checkpoint.rs`), `crates/skilllite-executor` (transcript, memory, plan storage under `chat_root()`), `crates/skilllite-sandbox`, `spec/architecture-boundaries.md`.
- Current behavior: Cross-step data at the LLM level is handled by existing `write_file`/`read_file` tools (writing to workspace). This works for moderate payloads but lacks run scoping, pluggable backends, and programmatic access from skill subprocess code. There is no unified **run-scoped artifact** abstraction.
- Key structural constraints discovered during review:
  - `AgentConfig` is `#[derive(Debug, Clone)]` — cannot hold `dyn` trait objects directly.
  - `ChatSession` holds `config: AgentConfig` plus runtime state (`data_root`, `session_key`, `skills`, etc.) — this is the natural injection point for the store.
  - In **run mode**, `RunCheckpoint.run_id` (UUID) identifies a run instance. In **chat mode**, only `session_key` + `session_id` exist; there is no `run_id`.
  - Existing executor storage (transcript, memory, plans) lives under `chat_root()` (`~/.skilllite/`). Artifacts should align with this convention or document a deliberate divergence.

## Architecture Fit

- Layer boundaries involved: **core** (trait + errors), **agent** (default `LocalDirArtifactStore` + injection into run/tool context). **Executor/sandbox** may later receive a resolved filesystem path or env var from the agent for subprocess skills—without depending on `agent` types.
- Interfaces to preserve: One-way dependency flow toward `core`; use explicit trait objects or config structs for cross-layer data, not concrete types from upper crates in lower crates.

## Dependency and Compatibility

- New dependencies: Prefer none; use `std::fs`, existing `skilllite-fs` helpers from `core`/`agent` as appropriate.
- Backward compatibility notes: Store is injected into `ChatSession` (not `AgentConfig`), so `AgentConfig` `Clone`/`Debug` invariants are preserved. Existing call sites that construct `ChatSession` gain an optional store parameter with a sensible default (local directory).

## Design Decisions

- Decision: **Trait and errors in `skilllite-core`; `LocalDirArtifactStore` in `skilllite-agent`.**
  - Rationale: Keeps `core` free of agent dependencies while allowing a single shared contract; matches discussion that production backends are user-supplied implementations of the same trait.
  - Alternatives considered: New crate `skilllite-artifacts` between core and agent.
  - Why rejected for v0: Extra crate overhead before demand; can split later if `executor` needs shared concrete helpers without pulling `agent`.
- Decision: **Inject store into `ChatSession`, not `AgentConfig`.**
  - Rationale: `AgentConfig` is `#[derive(Debug, Clone)]`; adding `Arc<dyn ArtifactStore>` would break both derives. `ChatSession` is the runtime context that already holds non-`Clone` state (tokio handles, caches). Store propagation flows: `ChatSession` → `agent_loop::run_agent_loop()` → tool execution context.
  - Alternatives considered: Add store to `AgentConfig` and remove `Clone`/`Debug`.
  - Why rejected: Would break all existing call sites that clone or debug-print `AgentConfig`.
- Decision: v0 **get/put** only; defer list/stream/TTL.
  - Rationale: Lowest cost path to unblock production adapters and local workflows.
  - Alternatives considered: Full blob store API in v0.
  - Why rejected: Scope and test burden; YAGNI until a concrete consumer needs it.
- Decision: v0 **put is always-overwrite** (upsert semantics).
  - Rationale: Simplest behavior; avoids concurrency/idempotency complexity in v0. Create-only / CAS can be added as an `opts` parameter later without breaking the base signature.
  - Alternatives considered: Create-only by default, or CAS.
  - Why rejected for v0: Adds error cases and test surface without a concrete consumer requesting it.
- Decision: v0 trait is **synchronous**.
  - Rationale: Matches existing `run_checkpoint` save/load pattern (sync I/O); `LocalDirArtifactStore` is naturally sync. Async callers (agent loop) use `spawn_blocking` or `block_in_place`.
  - Alternatives considered: `async-trait` from v0.
  - Why rejected for v0: Adds runtime complexity; local file I/O doesn't benefit. Migration path: a future v1 can introduce `AsyncArtifactStore` as a separate trait without breaking sync consumers.
- Decision: **No new LLM-callable tools in v0** (`artifact_put`/`artifact_get` not added).
  - Rationale: Existing `write_file`/`read_file` already handle LLM-mediated cross-step data. Adding dedicated artifact tools would overlap without clear benefit — LLM would have to choose between two ways to save a file, increasing confusion. Claude Code and OpenClaw also use generic file tools rather than artifact-specific tools for this purpose.
  - Alternatives considered: Register `artifact_put`/`artifact_get` as built-in tools (similar to `memory_write`/`memory_search` pattern).
  - Why rejected: Functional overlap with `write_file`/`read_file`; marginal value at LLM level. The real production gap is **programmatic access from skill subprocess code**, which LLM tools don't address.
- Decision: **Executor/sandbox wiring optional** in first delivery (v1 priority).
  - Rationale: Unblocks Rust-side contract and pluggable backend first; subprocess skills receiving `SKILLLITE_ARTIFACTS_DIR` or SDK is the **v1 deliverable** that unlocks production data flows.
  - Alternatives considered: Require `SKILLLITE_ARTIFACTS_DIR` in v0.
  - Why rejected: Couples v0 to sandbox runner changes and more review surface.
- Decision: **Chat mode artifact scope**: v0 uses `session_key` as fallback scope if no explicit `run_id` is set; artifacts in chat mode are **best-effort, not guaranteed cross-turn persistent** (chat mode may compact/clean artifacts across sessions).
  - Rationale: Run mode has a clear `run_id` lifecycle; chat mode scope is looser by nature. Avoid blocking v0 on designing full chat-mode artifact lifecycle.
  - Alternatives considered: No artifact support in chat mode for v0.
  - Why rejected: Would require gating artifact access by mode, adding complexity; `session_key` fallback is cheap.
- Decision: **Default artifacts root** under `chat_root()` (`~/.skilllite/`), e.g. `~/.skilllite/artifacts/<run_id>/`.
  - Rationale: Consistent with existing executor storage (transcripts, memory, plans all under `chat_root`). Discoverable for user debugging.
  - Alternatives considered: Workspace-local `.skilllite/artifacts/` directory.
  - Why rejected for default: Would scatter artifacts across workspaces; global root is more predictable for cross-workspace runs. Can be made configurable.

## Open Questions

- [ ] Exact sub-directory naming within `~/.skilllite/artifacts/<run_id>/`: flat keys with sanitization, or allow `/`-separated key hierarchies (e.g. `step1/output.json`)?
- [ ] How non-Rust skills receive `run_id` and artifact path in a follow-up (env var `SKILLLITE_ARTIFACTS_DIR` vs RPC method vs both).
- [ ] Should there be a cleanup / TTL policy for old run directories, or is that purely user/admin responsibility in v0?
- [ ] Propagation mechanism from `ChatSession` through `agent_loop` to individual tool handlers — explicit parameter on `run_agent_loop`, or shared context object (e.g. `RunContext` struct wrapping store + run_id)?
