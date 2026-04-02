# Review Report

## Scope Reviewed

- Files/modules:
  - `crates/skilllite-assistant/src/types/chat.ts`
  - `crates/skilllite-assistant/src/hooks/useChatEvents.ts`
  - `crates/skilllite-assistant/src/components/chat/MessageBubble.tsx`
  - `crates/skilllite-assistant/src/components/chat/MessageList.tsx`
  - `crates/skilllite-assistant/src/components/chat/SystemTimelineGroup.tsx`
  - `crates/skilllite-assistant/src/components/ChatView.tsx`
  - `crates/skilllite-agent/src/types/feedback.rs`
  - `crates/skilllite-agent/src/types/mod.rs`
  - `crates/skilllite-agent/src/agent_loop/helpers.rs`
  - `crates/skilllite-agent/src/agent_loop/execution.rs`
  - `crates/skilllite-agent/src/agent_loop/mod.rs`
  - `crates/skilllite-agent/src/extensions/builtin/chat_data.rs`
  - `crates/skilllite-agent/src/evolution.rs`
  - `crates/skilllite-agent/src/rpc.rs`
  - `crates/skilllite-agent/src/seed/execution.seed.md`
  - `crates/skilllite-agent/src/task_planner.rs`
  - `crates/skilllite-evolution/src/feedback.rs`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations.rs`
  - `crates/skilllite-evolution/src/lib.rs`
  - `README.md`
  - `docs/zh/README.md`
- Commits/changes:
  - Working tree changes for TASK-2026-017 (not committed).

## Findings

- Critical:
  - None.
- Major:
  - None.
- Minor:
  - `partial_success` detection currently requires explicit `partial_success=true` in structured tool output; non-structured partial outcomes are not auto-detected in this task.
  - Addressed in follow-up: added lightweight phrase-based fallback for non-explicit partial outputs.
  - `completion_type` is currently model-declared; semantic consistency still depends on prompt quality and may require future calibration.
  - Mitigated by this change: decision recording now stores both `reported` and `effective` completion type after structured-signal reconciliation.
  - Follow-up hardening applied: `complete_task` now rejects missing `completion_type` to remove silent default-success ambiguity.
  - Follow-up hardening applied: planning control now rejects `completion_type=success` when execution already has failure/replan signals.
  - Follow-up hardening applied: unfinished task plans no longer expose last assistant text as final result; response is normalized to explicit unfinished status.
  - Follow-up hardening applied: planning feedback completion type is normalized to `partial_success`/`failure` when task plan remains incomplete.
  - Follow-up hardening applied: assistant UI now also triggers evolution options from backend `done.completion_type`, preventing misses when partial/failure semantics appear only in final completion metadata rather than tool-result text.
  - Follow-up hardening applied: evolution-options dedupe in frontend is now turn-scoped (after latest user message), so repeated partial/failure outcomes across turns still surface actionable prompts.
  - Follow-up UX improvement: after user authorizes capability evolution, chat now shows explicit enqueue/progress notice (proposal id + where to monitor backend evolution status).
  - Follow-up UX improvement: authorized evolution card now includes live proposal progress UI (`status / acceptance_status`) with auto polling from backend proposal status API.
  - Follow-up UX improvement: evolution detail panel now includes `能力进化队列与执行` section backed by backlog query API to inspect per-proposal queue and execution state directly.
  - Follow-up behavior improvement: authorization now enqueues proposal and immediately triggers one background evolution run attempt (busy-safe via runtime skip semantics).
  - Follow-up UX improvement: queue rows now include `立即执行` manual trigger action for on-demand evolution run, with inline trigger-result diagnostics.
  - Follow-up behavior fix: manual trigger now passes forced proposal id into evolution runtime, avoiding false `nothing to evolve` when backlog has queued proposals but decisions queue is empty.
  - Follow-up behavior fix: dedupe-aware enqueue now returns the persisted backlog proposal id (instead of a transient generated id), preventing forced-trigger `proposal not found` failures after repeated authorizations of the same capability gap.
  - Follow-up resilience improvement: when forced proposal id is stale, runtime attempts deterministic recovery from authorization log linkage (`tool + outcome -> dedupe key`) before returning `NoScope`.
  - Follow-up reliability hardening: assistant manual-trigger path now executes evolution in-process (no external CLI subprocess dependency), eliminating binary version drift between desktop backend and installed `skilllite` command.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test` (in `crates/skilllite-assistant/src-tauri`)
  - `cargo test -p skilllite-evolution`
  - `cargo test`
  - `cd crates/skilllite-assistant && npm run build`
  - `cd crates/skilllite-assistant && npm run build` (after authorization progress notice UX update)
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `cd crates/skilllite-assistant && npm run build` (after proposal progress UI + polling)
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (after backlog queue API + detail section)
  - `cd crates/skilllite-assistant && npm run build` (after backlog queue section)
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (after immediate trigger-on-authorization)
  - `cd crates/skilllite-assistant && npm run build` (after immediate trigger-on-authorization)
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (after manual trigger command/UI)
  - `cd crates/skilllite-assistant && npm run build` (after manual trigger command/UI)
  - `cargo test -p skilllite-evolution`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (after forced-proposal execution fix)
  - `cd crates/skilllite-assistant && npm run build` (after forced-proposal execution fix)
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-evolution` (after dedupe-id return + stale-id recovery fix)
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (post-fix regression)
  - `cargo test` (workspace full regression)
  - `cargo fmt --check` (after in-process trigger refactor)
  - `cargo test -p skilllite-evolution`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` (after in-process trigger refactor)
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test` (workspace full regression after in-process trigger refactor)
  - `cd crates/skilllite-assistant && npm run build` (after turn-scoped dedupe optimization)
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite-agent` (after unfinished-plan response/completion-type normalization)
  - `cd crates/skilllite-assistant && npm run build`
  - `cargo test -p skilllite-evolution`
  - `cargo test`
- Key outputs:
  - All commands above completed successfully with passing test results.

## Decision

- Merge readiness: `ready`
- Follow-up actions:
  - Optional: add heuristic partial detection for non-structured tool outputs (e.g., language-only "currently unsupported").
