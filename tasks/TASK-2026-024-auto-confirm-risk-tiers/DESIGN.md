# Design: risk tiers and auto-approve

## Goals

- Stop **auto-approve** from approving destructive confirmations.
- Prefer **structured** `risk_tier` on the confirmation event instead of scraping `prompt` strings.
- Keep **deterministic** rules for v1; leave room for future LLM assist without blocking shipping.

## Proposed event shape (RPC → UI)

Today (conceptually): `confirmation_request` carries `{ "prompt": "..." }`.

Proposed payload (fields additive):

```json
{
  "prompt": "⚠️ Dangerous command detected\n\n...",
  "risk_tier": "confirm_required",
  "reason_code": "run_command.dangerous_pattern",
  "detail": "rm with force flag — may delete files irreversibly"
}
```

Suggested `risk_tier` values (v1):

| Value | Meaning | Auto-approve | Execute if user clicks Allow |
|-------|---------|--------------|-------------------------------|
| `low` | Routine tool confirm (e.g. generic run_command text with no dangerous match) | **Yes** (if setting on) | Yes |
| `confirm_required` | High impact or ambiguous (dangerous pattern match, key path write, network skill, L3 skill scan gate) | **No** | Yes (after manual confirm) |
| `blocked` | **Machine-wide** catastrophic only (fork bomb, explicit root-destruction patterns, future: `dd of=/dev/...`) | **No** | **No** — error returned |

**Note (product)**: Sensitive reads (e.g. `cat .env`, `source .env` via `run_command`) are **`confirm_required`**, not `blocked` — they risk secret exposure but are not “destroy the machine.” Today’s code hard-blocks these; implementing this task may **replace** that with a strong confirmation + `risk_tier: confirm_required`.

**Default if field missing** (old agent binary): treat as `confirm_required` so auto-approve stays safe.

## Behavioral matrix (design cases)

### Case A — `rm -rf /` style (blocked)

- **Input**: Command matches conservative blocked list (e.g. fork bomb, `rm -rf /` variants TBD in implementation).
- **Tier**: `blocked`
- **Auto-approve ON**: No auto action; user never gets an Allow path that runs the command — show error in tool result.
- **`SKILLLITE_HIGH_RISK_CONFIRM=none`**: Still **blocked** (no spawn).

### Case B — `rm -rf ./build` (confirm_required)

- **Input**: Matches “rm with force” dangerous pattern but not blocked list.
- **Tier**: `confirm_required`
- **Auto-approve ON**: Pending confirmation **stays** until user clicks Allow/Deny.
- **`none`**: Executes without prompt (current behavior) — **unless** later promoted to `blocked` by policy.

### Case C — `ls` (low)

- **Input**: No dangerous match; normal `run_command`.
- **Tier**: `low`
- **Auto-approve ON**: Auto-approve allowed.
- **`none`**: Executes without prompt.

### Case D — Write to key path (`package.json`, etc.)

- **Input**: `write_file` / `search_replace` hits `is_key_write_path`.
- **Tier**: `confirm_required` (same as today’s extra confirm, but now structured).
- **Auto-approve ON**: Manual confirm required.

### Case E — Network-enabled skill

- **Input**: Skill metadata `network.enabled` and first-time network confirm.
- **Tier**: `confirm_required`
- **Auto-approve ON**: Manual confirm required.

### Case F — Level 3 skill execution gate

- **Input**: Sandbox L3 scan / execution confirmation.
- **Tier**: `confirm_required` (or `low` only if product explicitly allows — default **confirm_required**).
- **Auto-approve ON**: Default manual confirm required.

### Case G — Sensitive file read via `run_command` (confirm_required)

- **Input**: Command matches sensitive-read patterns (e.g. `cat .env`, `source .env`, reading `.pem` / `.git/config` via shell).
- **Tier**: `confirm_required` with explicit copy that secrets may be exposed to the model/session logs.
- **Auto-approve ON**: Manual confirm required (never auto-approve).
- **`SKILLLITE_HIGH_RISK_CONFIRM=none`**: Product decision: either still require one-time confirm for this subclass, or allow skip — document clearly if parity with “no silent exfiltration” is required.

## UI notes

- Settings copy should state that auto-approve applies only to **low** tier confirmations.
- Optional: badge on confirmation bubble (“Manual required — high risk”) when tier ≠ `low`.

## Transcript / persistence

- Store `risk_tier` (and optional `reason_code`) on confirmation messages so restored sessions do not change policy mid-thread.
- If old transcripts lack tier, UI treats as `confirm_required` for display only; no retroactive auto-approve.

## Follow-ups (not in TASK-2026-024 scope)

- Align Evolution proposal `risk_level` with `risk_tier` for synthetic skills.
- Optional central policy crate consumed by CLI, MCP, and assistant.
