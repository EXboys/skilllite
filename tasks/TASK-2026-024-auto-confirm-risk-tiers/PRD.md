# PRD

## Background

Sensitive reads via `run_command` are implemented as **`confirm_required`** with mandatory confirmation (even when `SKILLLITE_HIGH_RISK_CONFIRM` omits `run_command` or is `none`). **`blocked`** covers machine-wide catastrophic patterns (fork bomb, root `rm`, `dd` to devices, `mkfs`, etc.). Desktop **auto-approve** applies only to `risk_tier: low`. Operators can still set `SKILLLITE_HIGH_RISK_CONFIRM=none` to skip **non-sensitive** `run_command` confirms only.

## Objective

1. Every user-visible confirmation request carries a **machine-readable risk tier** suitable for UI policy (auto-approve, manual only, or deny).
2. A **blocked** tier prevents execution regardless of auto-approve and regardless of `SKILLLITE_HIGH_RISK_CONFIRM=none` (for the covered operations).
3. Product and engineering share one **design reference** (`DESIGN.md`) for event shape and behavioral matrix.

## Functional Requirements

- **FR-1 — Structured tier**: Emit `risk_tier` (and optional `reason_code`, `source`) alongside `prompt` on the confirmation channel used by the desktop app (see DESIGN for JSON shape).
- **FR-2 — Auto-approve policy**: When `autoApproveToolConfirmations` is true, automatically approve **only** `low` tier (name finalized in implementation). Never auto-approve `confirm_required` or `blocked`.
- **FR-3 — Hard deny (`blocked` only)**: For tiers classified as **`blocked`** (machine-wide catastrophic), return a structured error without spawning the shell child. Sensitive-read commands use **`confirm_required`** instead of `blocked` unless a separate policy reclassifies them.
- **FR-4 — HIGH_RISK_CONFIRM interaction**:
  - If confirmation is skipped for a tool via `none`, **blocked** rules still apply for commands in the deny list.
  - Document this invariant in EN/ZH env reference.
- **FR-5 — Backward compatibility**: Older clients that ignore `risk_tier` still receive `prompt`; new clients must not break on missing tier (default to `confirm_required` for safety).

## Non-Functional Requirements

- **Security**: No new secret exfiltration paths; tier is derived from command/metadata already visible to the UI.
- **Performance**: Classification must be O(length of command) with trivial regex/set cost; no network calls.
- **Compatibility**: Transcript restore and session replay must store enough fields to render confirmation state correctly (or default safely).

## Constraints

- **Technical**: Follow `spec/structured-signal-first.md` — prefer structured fields over parsing localized `prompt` text in the UI.
- **Timeline**: Spec approval (`draft` → `ready`) before large implementation; incremental PRs acceptable (backend tier first, then UI).

## Success Metrics

- **Metric**: Auto-approve cannot approve a `blocked` or `confirm_required` confirmation in automated tests.
- **Baseline**: Current behavior auto-approves any confirmation (including dangerous prompt text).
- **Target**: 100% pass on targeted tests; zero reported false auto-approve for blocked fixtures in CI.

## Rollout

- **Rollout plan**: Land backend tier + defaults; then assistant gating; then expand deny list with tests per pattern.
- **Rollback plan**: Feature-flag or revert UI gating only (keep backend tier as additive) if false positives spike.

## Open decisions

- Exact enum values (`low` / `confirm_required` / `blocked` vs `high` / `critical`).
- Whether `confirm_required` sub-splits into `high` + `critical` for UX copy only.
- Source of truth for patterns (Rust-only vs generated shared asset).
- Behavior under `SKILLLITE_HIGH_RISK_CONFIRM=none` for **sensitive read** (`confirm_required`): still confirm vs allow silent execution (security vs power-user tradeoff).
