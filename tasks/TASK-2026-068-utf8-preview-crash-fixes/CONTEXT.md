# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-agent/src/agent_loop/helpers.rs`
  - `crates/skilllite-agent/src/llm/mod.rs`
- Current behavior:
  - Human evolution status slices `reason` at byte 47 when shortening long event reasons.
  - `update_task_plan` slices invalid string payload previews at byte 120.
  - `LlmClient::embed` slices unexpected response JSON previews at byte 500.
  - Each can panic if the byte boundary lands inside a multibyte UTF-8 character.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-commands` owns CLI presentation and should avoid panics while formatting persisted data.
  - `skilllite-agent` owns LLM/tool orchestration and should return structured errors on invalid inputs.
- Interfaces to preserve:
  - `skilllite evolution status` CLI behavior and JSON status output.
  - `ToolResult` error shape for planning-control validation.
  - `LlmClient::embed` return type and provider format support.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Preview text may be clipped at a safe boundary but remains diagnostic-only.

## Design Decisions

- Decision: Use existing UTF-8-safe helpers (`safe_truncate` or local char-boundary helper) instead
  of direct byte slices.
  - Rationale: Matches recent fixes and keeps changes local.
  - Alternatives considered: Convert all truncation sites in the repo.
  - Why rejected: The task is a critical bug fix and should avoid unrelated refactors.

## Open Questions

- [x] Does the change require docs sync? No, because flags, output schema, defaults, and documented
  command semantics are unchanged.
- [x] Are lower-severity skills-list issues included? No, they do not meet the crash/data-loss bar for
  this automation run.
