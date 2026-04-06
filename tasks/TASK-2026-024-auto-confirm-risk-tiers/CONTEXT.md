# CONTEXT

## Current implementation (baseline)

- **Auto-approve**: `ChatView.tsx` — only `risk_tier === low` confirmations auto-approve when the setting is on.
- **Confirmation transport**: Tauri bridge emits `skilllite-confirmation-request` with prompt text; response `skilllite_confirm` writes `{"method":"confirm","params":{"approved":bool}}` to agent stdin (`skilllite_bridge/chat.rs`).
- **run_command**: `crates/skilllite-agent/src/extensions/builtin/run_command.rs` — sensitive reads → mandatory **`confirm_required`** (not bypassed by `SKILLLITE_HIGH_RISK_CONFIRM=none`); `blocked` patterns hard-`bail`; routine confirms when `confirm_run_command` is enabled use `low` vs `confirm_required` by danger match.
- **Global confirm categories**: `crates/skilllite-agent/src/high_risk.rs` — `SKILLLITE_HIGH_RISK_CONFIRM` controls `write_key_path`, `run_command`, `network`.
- **Skill unified precheck**: Implemented in `skilllite-sandbox` as `security::run_skill_precheck` / `SkillPrecheckSummary` (SKILL.md heuristics + entry `ScriptScanner`). **Agent**: `skills/security.rs` delegates; `executor.rs` resolves `metadata_for_run` before confirm + hash; `run_in_sandbox_with_limits_and_level_opt` with `skip_skill_precheck: true` (host already gated). **CLI / `skilllite exec`**: runner runs the same precheck for L1–L3; gate via `SKILLLITE_AUTO_APPROVE`, non-TTY block, or stdin (`prompt_skill_precheck_continue`). **MCP `run_skill`**: same precheck; JSON `scan_kind: l3_skill_precheck` (legacy) + `scan_id`; runner `skip_skill_precheck` on Level 3 only.

## Technical boundaries

- **Agent / assistant contract**: Extending JSON on `confirmation_request` must remain backward compatible for older packaged assistants (unknown fields ignored).
- **EventSink**: `on_confirmation_request(&ConfirmationRequest) -> bool` with `risk_tier` (`Low` | `ConfirmRequired`) is implemented; desktop auto-approve only for `low`.
- **Tests**: Prefer unit tests on classification functions; integration tests optional if stdin protocol mocking is heavy.

## Dependencies

- None external; internal crates: `skilllite-agent`, `skilllite-assistant`, `skilllite-assistant` Tauri bridge, `skilllite-sandbox` (L3 precheck), `skilllite-core` for enums / observability.

## Classification source of truth

- **`run_command` L0/L1/L2**: Rust in `crates/skilllite-agent/src/extensions/builtin/run_command.rs` is authoritative. The assistant defaults unknown or missing `risk_tier` to **manual confirm** (`confirm_required` behavior). Additionally, **`scan_shell_command`** runs before `sh -c` (shell multi-stage / entropy / base64 heuristics); findings append to the confirm prompt as `confirm_required`.
- **Skill pre-spawn scan**: `run_skill_precheck` runs for **L1–L3** in the CLI runner when not skipped. The agent path runs it for all levels via `EventSink`, then always passes `skip_skill_precheck` into the runner.

## Compatibility

- Desktop-only auto-approve; CLI / headless flows may need env mirror (`SKILLLITE_AUTO_APPROVE_MAX_TIER`) — optional, document if added.
