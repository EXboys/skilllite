# Technical Context

## Current State

- Relevant crates/files: top-level `skilllite-assistant/` on `main` after #158
- Current behavior: Desktop is a standalone *directory* in the engine repo

## Architecture Fit

- Layer boundaries: engine vs optional GUI distribution
- Interfaces to preserve: L1 agent-rpc, L2 CLI --json, L3 files

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: `cd skilllite-assistant` inside the engine clone stops working after the cut; users clone the new repo

## Design Decisions

- Decision: `git subtree split --prefix=skilllite-assistant`
  - Rationale: preserves file history for the GUI tree
  - Alternatives considered: fresh orphan commit
  - Why rejected: loses history

## Open Questions

- [x] Can this agent create `EXboys/skilllite-assistant`? No (`createRepository` 403).
