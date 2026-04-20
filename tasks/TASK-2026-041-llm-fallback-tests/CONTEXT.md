# Technical Context

## Current State

- Relevant files/modules:
  - `crates/skilllite-assistant/src/utils/llmScenarioFallback.ts`
  - `crates/skilllite-assistant/scripts/test-llm-scenario-fallback.cjs`
  - `crates/skilllite-assistant/package.json`
- Previous behavior: fallback helper behavior was only validated by build/manual reasoning and occasional ad-hoc scripts.
- Implemented behavior: a committed focused Node test harness now exercises `buildScenarioCandidates()` and `runWithScenarioFallback()` through a lightweight TypeScript transpile-on-the-fly loader, without adding a heavyweight new test framework dependency.

## Architecture Fit

- Layer boundaries involved: assistant TypeScript only.
- Interfaces to preserve: public helper signatures and runtime behavior stay unchanged; tests live in `scripts/` and are invoked via npm.

## Dependency and Compatibility

- New dependencies: None (reuses existing `typescript` devDependency and Node built-ins).
- Backward compatibility notes: No runtime impact; test harness is dev-only.

## Design Decisions

- Decision: Use `node:test` + a tiny transpile loader instead of adding Vitest/Jest immediately.
  - Rationale: Minimal surface area and no new dependency churn for a focused helper-only suite.
  - Alternatives considered: Add a full test runner, or keep relying on ad-hoc verification scripts.
  - Why rejected: Full frameworks are heavier than needed for this helper scope; ad-hoc scripts are not integrated into a repeatable test command.

- Decision: Cover helper behavior directly.
  - Rationale: The core risk sits inside deterministic candidate selection, retry branching, and cooldown state.
  - Alternatives considered: Only caller-level tests.
  - Why rejected: Higher setup cost with less signal for the most failure-prone logic.

## Open Questions

- [ ] Whether future assistant utility tests should reuse this harness or justify adding a broader test framework.
- [ ] Whether a small integration test around `runWithScenarioFallbackNotified` is worth adding later.
