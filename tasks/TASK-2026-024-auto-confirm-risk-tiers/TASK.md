# TASK Card

## Metadata

- Task ID: `TASK-2026-024`
- Title: Risk-tiered confirmations and auto-approve policy
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-06`
- Target milestone: TBD (after PRD / DESIGN sign-off)

## Problem

Desktop **auto-approve** (`autoApproveToolConfirmations`) approves the first pending confirmation with no knowledge of severity. Backend `run_command` only **annotates** dangerous patterns in the prompt; it does not block them, and `SKILLLITE_HIGH_RISK_CONFIRM=none` can skip confirmation entirely for whole tool classes. There is no **risk tier** carried on the wire for the UI to enforce “auto-approve safe only.”

## Scope

- In scope:
  - Define a small **risk tier** model for confirmation requests (agent RPC → desktop assistant).
  - Policy for **auto-approve**: which tiers may be auto-approved; which require manual confirm; which are **hard-deny** (**`blocked` only** — machine-wide catastrophic; sensitive reads are **`confirm_required`**, not `blocked`, per stakeholder direction).
  - Align `run_command` dangerous/sensitive handling with that model (extend or replace regex-only warnings).
  - Documentation: EN/ZH env and user-facing hints where behavior changes.
  - Tests: Rust unit tests for classification; frontend tests or manual checklist if UI logic is non-trivial.
- Out of scope:
  - Full LLM-based command risk scoring (optional future); this task prefers **deterministic** rules first.
  - Changing default sandbox levels or MCP `confirmed=true` contract (unless required for parity).
  - Unifying Evolution `risk_level` with this tier model (listed as follow-up in DESIGN).

## Acceptance Criteria

- [x] Confirmation events include a **structured** `risk_tier` in addition to the human `prompt` (RPC `confirmation_request`, `RiskTier` in `skilllite-agent`).
- [x] Assistant **auto-approve** only when `risk_tier === low` (never auto-approves `confirm_required`; `blocked` does not reach the UI).
- [x] **`run_command` hard-deny** (fork bomb, `rm -rf /`): rejected **before** spawn; not bypassed by `SKILLLITE_HIGH_RISK_CONFIRM=none`.
- [x] **Sensitive `run_command` reads** (`.env`, `.key`, `.pem`, `.git/config`, `source .env`): **`confirm_required`** with strong prompt; always confirmed even when `SKILLLITE_HIGH_RISK_CONFIRM` omits `run_command` or is `none`.
- [x] `SKILLLITE_HIGH_RISK_CONFIRM` semantics documented: `none` must not bypass **blocked** tier (policy in PRD FR-4; explicit sentence in EN/ZH `ENV_REFERENCE.md` A11).
- [x] Shared or single-sourced rule list for `run_command` classification: **Rust authoritative** (`run_command.rs`); assistant treats missing/unknown `risk_tier` as `confirm_required` (see `types/chat.ts`). Optional JSON export / build-time sync deferred.
- [x] `python3 scripts/validate_tasks.py` passes; `tasks/board.md` reflects final status.

## Risks

- Risk: Regex-only classification misses variants (`sudo`, `/*`, chained commands).
  - Impact: False negatives → destructive commands slip through.
  - Mitigation: Tier `blocked` uses conservative patterns + tests; document residual risk; optional shell tokenizer follow-up.
- Risk: False positives block legitimate workspace commands.
  - Impact: User frustration; workarounds that weaken safety.
  - Mitigation: Start with narrow **blocked** set; keep `confirm_required` for ambiguous cases; telemetry/logging only if already aligned with privacy spec.

## Validation Plan

- Required tests: `cargo test` for affected crates (`skilllite-agent`, assistant bridge if touched).
- Commands to run: `cargo test -p skilllite-agent -- ...`, `python3 scripts/validate_tasks.py`.
- Manual checks: Toggle auto-approve; pending confirmations for mock `blocked` / `confirm_required` / `low` prompts behave per DESIGN matrix.

## Regression Scope

- Areas likely affected: `crates/skilllite-agent` (RPC events, `run_command`, `EventSink`), `crates/skilllite-assistant` (chat events, `ChatView` auto-approve), `skilllite_bridge` / Tauri chat stdin protocol, transcript persistence, i18n strings.
- Explicit non-goals: No unrelated refactors of agent loop or sandbox runner.

## Links

- Source TODO section: N/A (user request + codebase review).
- Related PRs/issues: TBD.
- Related docs: `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`, `spec/structured-signal-first.md`.
- Design cases: `tasks/TASK-2026-024-auto-confirm-risk-tiers/DESIGN.md`.
