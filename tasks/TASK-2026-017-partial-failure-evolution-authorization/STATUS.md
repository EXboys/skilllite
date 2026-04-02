# Status Journal

## Timeline

- 2026-04-01:
  - Progress:
    - Created task `TASK-2026-017-partial-failure-evolution-authorization`.
    - Drafted TASK/PRD/CONTEXT baselines and moved task status to `in_progress`.
    - Confirmed target implementation path: assistant chat events + tauri command + evolution backlog enqueue API.
  - Blockers:
    - None.
  - Next step:
    - Implement UI options prompt and backend authorization enqueue path.
- 2026-04-01:
  - Progress:
    - Added `evolution_options` chat message path in assistant UI for `partial_success` and `failure`.
    - Added multi-option prompt with explicit `【授权进化能力】` action.
    - Added tauri command `skilllite_authorize_capability_evolution` and bridge function `authorize_capability_evolution`.
    - Added `skilllite-evolution::enqueue_user_capability_evolution` API to queue governed backlog proposals.
    - Added regression test for capability-evolution backlog enqueue.
    - Synced user-facing docs in `README.md` and `docs/zh/README.md`.
    - Completed verification commands (`fmt`, `clippy`, tauri tests, evolution tests, workspace tests).
  - Blockers:
    - None.
  - Next step:
    - Keep task in `done`; collect UX feedback for future heuristic partial-detection improvement.
- 2026-04-01:
  - Progress:
    - Added lightweight fallback rules for `partial_success` detection in `useChatEvents`.
    - Introduced a maintainable signal phrase list (CN/EN) to classify subset-fulfilled outcomes.
    - Kept structured-field detection as highest-priority decision path.
    - Verified frontend build succeeds (`npm run build`).
  - Blockers:
    - None.
  - Next step:
    - Monitor false positives and refine phrase list if needed.
- 2026-04-01:
  - Progress:
    - Extended model-side structured completion signal: `complete_task` now accepts optional `completion_type`.
    - Added `TaskCompletionType` into `ExecutionFeedback` and propagated to RPC done event payload.
    - Added `completion_type` persistence in evolution `decisions` table with migration (`ALTER TABLE ... ADD COLUMN completion_type`).
    - Updated conversion path `ExecutionFeedback -> DecisionInput`.
    - Added completion consistency reconciliation before decision insertion:
      reported completion is cross-checked with structured failure/replan/task-completed signals.
    - Added dual decision fields:
      `completion_type` (effective) + `completion_type_reported` (model-reported).
    - Added/updated regression tests for `complete_task` completion type and evolution decision insertion.
    - Updated execution prompt guidance to document `completion_type` semantics.
  - Blockers:
    - None.
  - Next step:
    - Observe real sessions and verify LLM uses `completion_type=partial_success` for subset-fulfilled tasks.
- 2026-04-01:
  - Progress:
    - Enforced hard contract on `complete_task`: `completion_type` is now required (missing value returns error).
    - Added runtime declaration-validation gate in planning control:
      when failure/replan signals already exist, `completion_type=success` is rejected and must be downgraded to `partial_success` or `failure`.
    - Updated `complete_task` schema required fields to include `completion_type`.
    - Updated planner/task prompt guidance to require explicit `complete_task(task_id, completion_type=...)`.
    - Updated regression tests in `helpers.rs` and `execution.rs` for required completion type and control-tool auto-recovery compatibility.
    - Re-ran format/lint/test gates and confirmed all pass.
  - Blockers:
    - None.
  - Next step:
    - Observe production logs for `success` overuse and continue consistency calibration.
- 2026-04-01:
  - Progress:
    - Fixed unfinished-task final response behavior in planning loop result assembly:
      when task plan is still pending, final response is now an explicit unfinished summary instead of the last assistant free-form text.
    - Added final completion-type normalization in planning loop:
      if tasks are not completed, completion type is downgraded to `partial_success` or `failure` based on execution facts.
    - Added regression tests for unfinished-plan final response and updated existing build-agent-result completion-path test.
    - Re-ran `cargo test -p skilllite-agent` and confirmed all tests pass.
  - Blockers:
    - None.
  - Next step:
    - Observe weather/forecast-like partial-capability sessions to ensure logs no longer present unresolved content as final completion.
- 2026-04-01:
  - Progress:
    - Added `done`-event completion handling in assistant chat hook:
      when backend reports `completion_type=partial_success|failure`, UI now raises capability-evolution options even if tool-result text itself lacks explicit partial signals.
    - Refactored evolution-options insertion into shared helper to keep dedupe behavior consistent across `tool_result` and `done` paths.
    - Verified frontend build passes (`npm run build` in `crates/skilllite-assistant`).
  - Blockers:
    - None.
  - Next step:
    - Validate with weather-forecast gap scenario that partial-success popup appears reliably.
- 2026-04-01:
  - Progress:
    - Confirmed backend `tool_result` dedupe already scopes by turn (`turn_id + tool + result + is_error`), not global session-only content.
    - Optimized frontend evolution-options dedupe to current-turn scope:
      duplicate suppression now checks only messages after the latest user turn, allowing cross-turn repeated prompts for repeated partial/failure outcomes.
    - Verified frontend build and lint checks pass.
  - Blockers:
    - None.
  - Next step:
    - Re-run repeated weather-query sessions and confirm each new turn can surface a fresh evolution prompt.
- 2026-04-01:
  - Progress:
    - Clarified and aligned UX semantics for `【授权进化能力】`:
      authorization enqueues backlog proposal (queued) and does not imply immediate foreground execution in the same chat turn.
    - Added chat-level progress feedback after authorization:
      UI now displays queued proposal id and a direct hint to monitor progress in `自进化 > 详情与审核`.
    - Added best-effort status snapshot append (unprocessed decisions, pending skills, last run timestamp) after enqueue success.
    - Verified frontend build and lint checks pass.
  - Blockers:
    - None.
  - Next step:
    - Validate end-to-end UX that users can see immediate enqueue acknowledgement and then track evolution progress in side panel.
- 2026-04-02:
  - Progress:
    - Added proposal-level progress query API for assistant UI:
      tauri command `skilllite_get_evolution_proposal_status` now returns backlog status by `proposal_id`.
    - Added chat progress UI on evolution authorization message:
      authorized `evolution_options` card now shows proposal id, live status (`status / acceptance_status`), updated timestamp, and note.
    - Added auto-polling after authorization:
      chat polls proposal status every 5 seconds (up to ~2 minutes) and auto-stops on terminal states (`executed`, `policy_denied`, `blocked`, `failed`, `archived`).
    - Verified `cargo test` for assistant tauri crate and frontend build.
  - Blockers:
    - None.
  - Next step:
    - Validate in real sessions that queued proposals transition visibly to executing/executed states in the same chat card.
- 2026-04-02:
  - Progress:
    - Added backlog queue API for assistant UI:
      tauri command `skilllite_load_evolution_backlog` now returns latest proposal rows (`proposal_id`, `status`, `acceptance_status`, `risk`, `roi`, `updated_at`, `note`).
    - Added explicit `能力进化队列与执行` section in `自进化与审核` detail view so users can inspect queue/execution states directly.
    - Verified assistant tauri tests and frontend build after queue section integration.
  - Blockers:
    - None.
  - Next step:
    - Validate that authorized proposals move from `queued` to later states on subsequent scheduler runs.
- 2026-04-02:
  - Progress:
    - Changed authorization behavior to trigger immediate background evolution run attempt after enqueue:
      if runtime is idle, authorized backlog is processed sooner; if runtime is busy, evolution run returns busy and queue remains intact.
    - Added evolution event logging for trigger attempt result (`capability_evolution_trigger_run`) with concise command output snapshot.
    - Verified assistant tauri tests and frontend build after command signature changes.
  - Blockers:
    - None.
  - Next step:
    - Observe authorized proposals transition latency and confirm busy-skip behavior is visible in event stream.
- 2026-04-02:
  - Progress:
    - Added manual trigger action in backlog queue UI (`立即执行`) for each proposal row.
    - Added tauri command `skilllite_trigger_evolution_run` and bridge integration to invoke `skilllite evolution run --json` on demand.
    - Added trigger result feedback text in detail panel for quick diagnosis (busy/no-scope/executed output).
    - Verified assistant tauri tests and frontend build.
  - Blockers:
    - None.
  - Next step:
    - Validate user flow for manual trigger from queued proposal row and observe state transition in queue list.
- 2026-04-02:
  - Progress:
    - Fixed manual trigger semantics to honor selected proposal id:
      `skilllite_trigger_evolution_run` now passes `SKILLLITE_EVO_FORCE_PROPOSAL_ID`, and evolution runtime loads/executes that backlog proposal directly instead of relying only on decision-derived candidates.
    - Added backlog proposal loader/parsing path in evolution runtime to map backlog row data into `EvolutionProposal` for forced execution.
    - Kept busy-safe behavior unchanged (`SkippedBusy` still prevents concurrent evolution runs).
    - Verified `cargo test -p skilllite-evolution`, assistant tauri tests, and frontend build.
  - Blockers:
    - None.
  - Next step:
    - Re-run selected proposal execution in UI and verify status transitions from queued to executing/executed for the targeted proposal id.
- 2026-04-02:
  - Progress:
    - Diagnosed repeated `Evolution: nothing to evolve` after manual trigger for proposal ids shown in UI.
    - Root cause identified: capability-evolution enqueue used dedupe upsert but still returned the newly generated proposal id even when insert was ignored, producing stale/non-existent ids for later forced execution.
    - Fixed enqueue return semantics in `skilllite-evolution`:
      now returns the actual non-executed backlog row id selected by dedupe key.
    - Added forced-trigger recovery path:
      when a forced proposal id is missing, evolution now attempts to recover the real backlog proposal via `capability_evolution_authorized` log linkage (`tool + outcome -> dedupe key`) before declaring `NoScope`.
    - Added regression test `enqueue_user_capability_evolution_returns_existing_id_when_deduped`.
    - Re-verified with `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p skilllite-evolution`, assistant tauri tests, and full `cargo test`.
  - Blockers:
    - None.
  - Next step:
    - Ask user to click `立即执行` again on backlog rows and verify targeted proposal transitions from `queued` to `executing/executed` (or explicit policy-denied/blocked state with note).
- 2026-04-02:
  - Progress:
    - Replaced manual trigger execution path in assistant bridge from external CLI subprocess to in-process evolution runtime call.
    - `skilllite_trigger_evolution_run` now invokes `skilllite_evolution::run_evolution(...)` directly via `skilllite-agent` LLM adapter, while keeping forced proposal semantics.
    - Manual trigger no longer depends on `~/.skilllite/bin/skilllite` version parity, avoiding desktop-vs-CLI binary drift.
    - Preserved diagnostics text shape (`Evolution: nothing to evolve` hints) and event logging (`manual_evolution_run_triggered`) for UI compatibility.
    - Added assistant-tauri crate dependency wiring for `skilllite-agent` and runtime support, then re-verified full test gates.
  - Blockers:
    - None.
  - Next step:
    - Validate with user-reported proposal id in UI that manual trigger now runs current workspace code path end-to-end.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
