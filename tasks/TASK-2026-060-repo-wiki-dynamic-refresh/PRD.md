# PRD

## Background

Repo Wiki now has deterministic compile/query/lint commands, but freshness depends on manual compile execution. Qoder-like dynamic behavior needs a mechanical stale check and safe automatic refresh at user-triggered entry points.

## Objective

Make Repo Wiki feel dynamic without a background watcher: source changes are detected from content fingerprints and refreshed when users ingest or query wiki knowledge.

## Functional Requirements

- FR-1: Compiled wiki articles must store source fingerprints so stale state can be detected later.
- FR-2: Add `skilllite wiki status` to report freshness without writing files.
- FR-3: `skilllite wiki ingest` must compile by default and expose `--no-compile`.
- FR-4: `skilllite wiki query` must refresh stale wiki content before searching and expose `--no-compile`.
- FR-5: Manual `skilllite wiki compile` remains supported.

## Non-Functional Requirements

- Security: Do not add background execution, network access, or hidden writes outside `.skilllite/wiki/`.
- Performance: Use scoped filesystem scans and simple deterministic fingerprints.
- Compatibility: Preserve existing commands; new behavior is additive with skip flags.

## Constraints

- Technical: Avoid new dependencies unless already present; keep commands crate dependency direction unchanged.
- Timeline: Implement deterministic dynamic refresh only, not LLM-backed research.

## Success Metrics

- Metric: A modified raw source is reported stale and refreshed before query.
- Baseline: Query can read stale compiled articles unless manual compile is run.
- Target: Unit tests prove stale detection and automatic refresh.

## Rollout

- Rollout plan: Additive CLI behavior under `skilllite wiki`.
- Rollback plan: Users can pass `--no-compile`; code can revert status/auto-refresh while preserving Markdown data.
