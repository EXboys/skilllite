# PRD

## Background

Skill-based and super-agent workflows increasingly span **multiple steps** and **external systems**. Today, small local tasks can rely on informal file usage; **production** scenarios need predictable **run-scoped** artifact persistence, observability, and the option to back storage with enterprise systems (object stores, internal gateways, databases) without rewriting orchestration logic.

## Objective

Deliver a **v0 run-scoped artifact storage contract** in the Rust workspace: a **core-level interface**, a **default local directory implementation** in the agent layer, and **injection** into the agent run lifecycle — establishing the architectural foundation for pluggable backends.

v0 does **not** add new LLM-callable tools; existing `write_file`/`read_file` remain the LLM-level mechanism for cross-step data. The real production value comes in **v1**: exposing the artifact store to **skill subprocess code** (Python etc.) via env var or SDK so that high-volume, multi-system data flows can use a unified, pluggable storage layer programmatically.

## Functional Requirements

- FR-1: **Core contract** — `ArtifactStore` (or equivalent name) supports at least **get** and **put** for bytes (or documented payload type) scoped by **`run_id`** and a logical **`key`**; failures are represented as **structured errors** (e.g. not found, invalid key, backend failure with retry hint where applicable). v0 scope is **run-level** only; `step_id` granularity is deferred unless a concrete consumer requires it.
- FR-2: **Default implementation** — `LocalDirArtifactStore` in `skilllite-agent` implements the core trait using a configurable base directory layout; keys are validated to prevent path traversal; writes are **atomic** where the platform allows.
- FR-3: **Injection** — The active agent run holds the store (and `run_id`) in `ChatSession` (runtime context, not `AgentConfig`). The store is propagated through the agent loop so that future consumers (v1 subprocess wiring, potential new tools) have a defined access path. v0 does **not** require new LLM tools — existing `write_file`/`read_file` handle LLM-mediated cross-step data.
- FR-4: **Extensibility** — Third parties or product code can provide alternative implementations of the same trait (e.g. S3-backed) without forking the agent loop.

## Non-Functional Requirements

- Security: Safe resolution of artifact paths; no raw user `key` segments as unchecked filesystem components; align with existing path validation practices in the repo.
- Performance: v0 targets moderate payloads in-process; no requirement for streaming in v0. Trait is **synchronous** in v0 (matching existing `run_checkpoint` patterns); async callers use `spawn_blocking`.
- Compatibility: Preserve `spec/architecture-boundaries.md` dependency direction (`entry → commands → agent → executor → sandbox → core`); `core` must not depend on `agent`.

## Constraints

- Technical: Trait lives in `skilllite-core`; concrete local store in `skilllite-agent`; lower layers must not import `agent` types.
- Timeline: v0 focuses on **mechanism + local default**; executor/sandbox env wiring for subprocess skills is the **v1 priority** (the real production unlock). No new LLM tools in v0.

## Success Metrics

- Metric: Developers can replace storage by implementing `ArtifactStore` without changing orchestration APIs beyond configuration/injection.
- Baseline: No shared contract; ad-hoc files per skill; cross-step data via `write_file`/`read_file` (works but no scoping, no pluggable backend).
- Target: One documented trait, one in-tree local implementation, one injection path on the agent run. LLM-level cross-step data continues via existing file tools; the contract enables v1 subprocess SDK and pluggable backends.

## Rollout

- Rollout plan: Land behind existing CLI/RPC entrypoints; default to `LocalDirArtifactStore` with a documented directory layout under the user/workspace data root (exact path TBD in implementation).
- Rollback plan: Feature-flag or config to disable artifact usage in tools until consumers migrate; document no-op or fallback behavior if store is unset (only if explicitly chosen in design).
