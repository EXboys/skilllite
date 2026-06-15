# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/llm/mod.rs`
  - `crates/skilllite-agent/src/task_planner.rs`
  - `crates/skilllite-agent/src/prompt.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`
- Current behavior:
  - `LlmClient::embed` and `TaskPlanner::parse_task_list` build debug/error
    previews with byte slicing.
  - Desktop status/backlog/authorization paths pass `--workspace`, but life-pulse
    growth and the detached authorized-capability run do not.
  - `get_skill_full_docs` applies a security notice to high-risk `SKILL.md`
    content only; references and bash-tool up-front docs are not covered.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-agent` owns prompt construction and LLM/task-planner handling.
  - The Tauri desktop bridge owns subprocess argument construction for the CLI.
  - `skilllite-core` already owns high-risk skill doc pattern detection.
- Interfaces to preserve:
  - Existing CLI subcommand and flag names.
  - Existing `LoadedSkill` and metadata structures.
  - Existing evolution DB schema and feedback APIs.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - Desktop subprocesses still run the same commands, with explicit workspace
    added to align execution with existing UI reads/writes.
  - Prompt content gains the existing security notice only for already-detected
    high-risk patterns.

## Design Decisions

- Decision: Use `safe_truncate` for all affected preview strings.
  - Rationale: It is the local helper already used by the recent UTF-8 fixes.
  - Alternatives considered: Ad hoc `char_indices` helpers in each file.
  - Why rejected: More duplication and higher risk than reusing the established helper.
- Decision: Pass `--workspace <workspace>` to desktop background evolution runs.
  - Rationale: The CLI now resolves DB paths from explicit workspace, and UI
    read/enqueue paths already pass it.
  - Alternatives considered: Change current directory or set only environment.
  - Why rejected: Current directory and env resolution already diverged in the
    regression scenario.
- Decision: Reuse `SKILL_MD_SECURITY_NOTICE` for high-risk reference/bash docs.
  - Rationale: The policy text already exists for the same threat model.
  - Alternatives considered: Add a new warning or block references.
  - Why rejected: New policy wording/docs are unnecessary for this minimal fix.

## Open Questions

- [x] Is a docs sync required? No public command/env/security policy semantics are
  changed; this only applies existing notices to missed prompt injection paths.
- [x] Are schema or migrations involved? No.
