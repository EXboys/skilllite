# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/chat_session.rs` (`summarize_for_memory`)
  - `crates/skilllite-agent/src/extensions/registry.rs` / `memory.rs` (search uses `"default"`)
  - `crates/skilllite-executor/src/memory.rs` (`index_path`)
  - `crates/skilllite-commands/src/migrate/openclaw.rs` (`collect_env_from_openclaw_json`)
- Current behavior:
  - Clear indexes `memory/{session_key}.sqlite`.
  - Provider loop inserts every `apiKey` as `OPENAI_API_KEY`.

## Architecture Fit

- Layer boundaries involved: agent chat session → executor memory index; commands migrate → allowlisted env merge.
- Interfaces to preserve: `index_path(chat_root, agent_id)` signature; `SECRET_ENV_ALLOWLIST`.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: search never read per-session sqlite files. Unknown OpenClaw provider keys no longer become `OPENAI_API_KEY`.

## Design Decisions

- Decision: Index clear-session memory under `"default"` instead of `session_key`.
  - Rationale: Markdown is already a shared daily file; search/write tools hardcode `"default"`.
  - Alternatives considered: Thread session key through all memory tools.
  - Why rejected: Coordinated redesign; not a minimal fix.
- Decision: Map known OpenClaw provider ids to allowlisted env keys; skip unknown ids.
  - Rationale: Prevents last-wins credential corruption.
  - Alternatives considered: Keep writing everything to `OPENAI_API_KEY`.
  - Why rejected: Breaks working OpenAI keys when another provider is listed last.

## Open Questions

- [x] Should unknown providers map to `OPENAI_API_KEY`? No — fail closed.
