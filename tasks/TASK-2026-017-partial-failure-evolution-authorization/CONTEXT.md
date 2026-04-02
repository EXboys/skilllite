# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-assistant/src/hooks/useChatEvents.ts`
  - `crates/skilllite-assistant/src/components/chat/MessageBubble.tsx`
  - `crates/skilllite-assistant/src/components/ChatView.tsx`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations.rs`
  - `crates/skilllite-evolution/src/lib.rs`
- Current behavior:
  - Confirmation flow is boolean only (`allow/deny`) from backend `confirmation_request`.
  - Clarification flow is suggestion-based and tied to agent-side `clarification_request`.
  - No dedicated user action to directly enqueue capability evolution proposal from chat tool outcomes.
  - Legacy `complete_task` semantics were binary (done/not-done) without partial/failure completion class.

## Architecture Fit

- Layer boundaries involved:
  - Assistant UI -> Tauri command -> bridge integration -> evolution crate.
- Interfaces to preserve:
  - Existing confirmation/clarification channels and payload formats.
  - Evolution coordinator policy runtime and backlog schema compatibility.
- Existing task-plan state transitions and event payload compatibility.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Additive changes only; existing command contracts remain valid.

## Design Decisions

- Decision: Introduce a new chat message type for capability options instead of overloading clarification flow.
  - Rationale: Clarification state is bound to active agent await state; this feature should be user-driven from tool result handling.
  - Alternatives considered: Reuse clarification messages and call `skilllite_clarify`.
  - Why rejected: No pending clarification channel in this scenario; action would be dropped.
- Decision: Enqueue proposal through a dedicated evolution API helper.
  - Rationale: Keep enqueue semantics centralized in `skilllite-evolution`.
  - Alternatives considered: Direct SQL insertion in assistant bridge.
  - Why rejected: Violates ownership and increases schema drift risk.
- Decision: Make `completion_type` mandatory on `complete_task` with explicit `success|partial_success|failure`.
  - Rationale: Enforce explicit completion declaration and prevent silent default-to-success on partial/failure outcomes.
  - Alternatives considered: Keep completion binary and classify only from tool-result text.
  - Why rejected: Domain-specific heuristics are brittle and disconnected from planner completion semantics.

## Open Questions

- [ ] Should `partial_success` support heuristic fallback when tool does not emit explicit `partial_success=true`?
- [ ] Should proposal priority differ by source tool domain in a follow-up task?
