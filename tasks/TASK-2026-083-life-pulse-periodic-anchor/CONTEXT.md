# CONTEXT — Life Pulse periodic growth anchor

## Technical boundaries

- Mutation lives in `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/growth.rs`.
- Schedule math remains in `skilllite_evolution::growth_schedule::{inspect_growth_due, growth_due}`.
- Status continues to be loaded via `skilllite evolution status --json --periodic-anchor-unix <n>`.

## Compatibility

- Matches agent-rpc behavior: `growth_due` seeds on first call and advances when `need_periodic` is true, even if the caller later skips spawn for empty proposals.

## Constraints

- Prefer unlocking the mutex during the status subprocess to avoid blocking UI reads of `periodic_anchor_unix`.
