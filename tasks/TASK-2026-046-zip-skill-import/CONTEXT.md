# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/skill/add/mod.rs`
  - `crates/skilllite-commands/src/skill/add/source.rs`
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/skill.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`
- Current behavior:
  - `skilllite add` supports git-style URLs, ClawHub slugs, and local directories.
  - `fetch_from_clawhub()` already extracts ZIP payloads safely, but that code is
    scoped to the ClawHub source path.
  - Desktop skill install already shells out to `skilllite add`, so adding local
    ZIP support there automatically benefits the desktop backend.

## Architecture Fit

- Layer boundaries involved:
  - Entry: `skilllite` clap/dispatch wiring.
  - Commands: `skilllite-commands::skill::add`.
  - Core: existing manifest / metadata / discovery APIs only.
- Interfaces to preserve:
  - `skilllite_commands::skill::cmd_add(...)`
  - Desktop bridge `add_skill(workspace, source, force, skilllite_path)`
  - Existing add-source parsing for local directories and git URLs.

## Dependency and Compatibility

- New dependencies:
  - Likely reuse `zip` in `skilllite-commands` directly for local archive extraction.
- Backward compatibility notes:
  - Command shape should remain `skilllite add <SOURCE>`.
  - Existing non-ZIP local paths must continue to resolve as directories.

## Design Decisions

- Decision: Extend `skilllite add` source parsing instead of adding a separate
  `import-zip` command.
  - Rationale: Desktop already calls `skilllite add`; reusing it avoids a second
    install API and preserves the existing manifest/admission flow.
  - Alternatives considered: Add a dedicated `import-zip` subcommand.
  - Why rejected: More surface area, duplicated install pipeline, no immediate
    desktop benefit over extending `add`.
- Decision: Support local ZIP paths first, not remote ZIP URLs.
  - Rationale: Desktop can download files itself and pass a local path; this keeps
    network policy unchanged in the first step.
  - Alternatives considered: Add direct HTTP ZIP download in `cmd_add`.
  - Why rejected: Larger scope, more network and trust policy questions.

## Open Questions

- [ ] Should remote ZIP URLs be parsed as a future `add` source type or stay in the desktop layer?
- [ ] Do we want a structured `source_type=zip_local` in manifests, or is the original path string sufficient for now?
