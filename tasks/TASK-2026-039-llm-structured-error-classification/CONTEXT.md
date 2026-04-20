# Technical Context

## Current State

- Relevant files/modules:
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/llm_routing_error.rs`
  - `crates/skilllite-assistant/src-tauri/src/lib.rs`
  - `crates/skilllite-assistant/src/utils/llmScenarioFallback.ts`
  - non-streaming callers in `ChatView.tsx` and `EvolutionSection.tsx`
- Previous behavior: `isRetryableLlmError()` scanned raw strings for phrases like `429`, `timeout`, or `network`.
- Implemented behavior: routing-aware Tauri commands now return `LlmInvokeResult<T>` with `LlmRoutingError { kind, retryable, message }`; the assistant unwraps that envelope and only falls back to raw-string heuristics for compatibility.

## Architecture Fit

- Layer boundaries involved: Rust/Tauri bridge produces the structured routing signal; assistant TypeScript consumes it.
- Interfaces to preserve: existing command names stay the same; only their success payload shape changes for the routing-aware callers already under assistant control.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: The frontend still keeps raw-string retry heuristics as a fallback for any older/unstructured paths.

## Design Decisions

- Decision: Put the structured classification at the Tauri bridge boundary.
  - Rationale: This keeps retry semantics close to the Rust-side source while avoiding a broader agent-crate error refactor.
  - Alternatives considered: Expand string matching in the UI only; refactor `skilllite-agent` to expose a new typed LLM error all the way up.
  - Why rejected: UI-only heuristics remain fragile; full agent-crate refactor is higher blast radius than needed for this step.

- Decision: Classify in layers.
  - Rationale: Prefer explicit local/configuration branches and HTTP status extraction first; keep narrow network phrase matching only as a final compatibility fallback.
  - Alternatives considered: Require every failure path to emit a typed source variant immediately.
  - Why rejected: Too invasive for the current assistant/Tauri scope.

## Open Questions

- [ ] Whether to generalize this envelope to more invoke commands beyond routing-aware LLM calls.
- [ ] Whether to push the status/error kind into `skilllite-agent` itself later for even stronger semantics.
