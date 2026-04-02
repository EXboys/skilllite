# TASK Card

## Metadata

- Task ID: `TASK-2026-017`
- Title: Partial/Failure Evolution Authorization Options
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

When a tool partially satisfies a request (`partial_success`) or fails (`failure`), the current UI does not offer an explicit user-controlled recovery/evolution decision path.
As a result, capability gap handling is delayed to periodic evolution and user intent ("authorize capability evolution now") is not captured as structured backlog input.

## Scope

- In scope:
  - Add assistant UI multi-option decision prompt for `partial_success` and `failure` tool outcomes.
  - Include the explicit option `【授权进化能力】` in that prompt.
  - Add a backend bridge command to enqueue a user-authorized evolution backlog proposal.
  - Keep risk bounded: enqueue proposal only; do not force immediate high-risk execution.
- Out of scope:
  - Redesigning agent internal clarify protocol.
  - Automatic capability synthesis execution in the same turn.
  - New risk-policy model changes.

## Acceptance Criteria

- [ ] For tool outcomes classified as `partial_success` or `failure`, UI shows a multi-option prompt with `【授权进化能力】` as one option.
- [ ] Choosing `【授权进化能力】` sends a structured request to backend and creates/updates a backlog proposal entry.
- [ ] Existing confirmation and clarification flows continue to work without regression.
- [ ] `complete_task` rejects calls without `completion_type`, and planner prompts require explicit `completion_type`.

## Risks

- Risk: Over-triggered prompts may create user noise.
  - Impact: Reduced chat UX quality.
  - Mitigation: Trigger only for explicit failure (`is_error` / `success=false`) and explicit partial markers (`partial_success=true`), with dedupe guard.
- Risk: Unauthorized automatic evolution execution.
  - Impact: Unexpected behavior changes.
  - Mitigation: Backend writes backlog proposal only; existing coordinator policy/shadow mode remains authoritative.

## Validation Plan

- Required tests:
  - `skilllite-assistant` frontend/bridge unit tests (affected scope)
  - `skilllite-evolution` tests for backlog enqueue helper
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-assistant`
  - `cargo test -p skilllite-evolution`
  - `cargo test`
- Manual checks:
  - Simulate `failure` tool result and verify option prompt appears.
  - Simulate `partial_success` tool result and verify option prompt appears.
  - Click `【授权进化能力】` and verify backlog reflects new queued proposal.

## Regression Scope

- Areas likely affected:
  - `skilllite-assistant` chat event handling and bubble rendering
  - tauri invoke bridge for chat/evolution
  - evolution backlog insertion path
- Explicit non-goals:
  - High-risk autonomous capability execution in the same turn

## Links

- Source TODO section:
  - `todo/12-SELF-EVOLVING-ENGINE.md` (section around P7 capability gap/evolution governance)
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
