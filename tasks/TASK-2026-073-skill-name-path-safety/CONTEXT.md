# CONTEXT

## Technical Boundaries

- Crate: `skilllite-core` owns the shared name validator.
- Consumers: `skilllite-commands` (add/remove/find_skill/update) and `skilllite` MCP handlers.
- No sandbox runtime policy changes; this is filesystem path safety for skill identity strings.

## Constraints

- Must remain compatible with existing single-segment skill directory names, including non-ASCII.
- Must not depend on path existence (validation happens before join/create).
- Keep fix minimal: no skill rename UX, no manifest schema changes.

## Compatibility Notes

- Callers that previously accepted `../x` or absolute names will now receive validation errors. That is intentional fail-closed behavior.
- PR #89 introduces a similar pending-skill validator in evolution; this task covers install/MCP/CLI lookup paths that #89 does not.

## Related Code

- `crates/skilllite-commands/src/skill/add/mod.rs` install destination selection
- `crates/skilllite-commands/src/skill/common.rs` `find_skill`
- `crates/skilllite-commands/src/skill/remove.rs`
- `skilllite/src/mcp/handlers.rs` `get_skill_info` / `run_skill`
