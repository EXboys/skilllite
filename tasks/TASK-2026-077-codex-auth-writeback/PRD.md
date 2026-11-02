# PRD

## Background

Codex stores the ChatGPT session in `CODEX_HOME/auth.json` and refreshes it under an exclusive `auth.lock`. SkillLite's subscription sandbox copies that file into an ephemeral home and, before this change, always copied it back on exit.

## Objective

A Codex sandbox run must persist a changed `auth.json` and must leave the host file untouched when the sandbox copy is still the snapshot from startup.

## Functional Requirements

- FR-1: After copying `.codex`, remember the bytes of `auth.json` when that regular file exists.
- FR-2: On exit, cancel, or stdout error, write `auth.json` back only when the ephemeral file is a regular file whose bytes differ from that snapshot.
- FR-3: Take a blocking exclusive `flock` on `.codex/auth.lock` for the duration of a writeback. Do not create that lock when skipping an unchanged file.
- FR-4: Keep refusing a non-regular ephemeral `auth.json`, including a symlink.

## Non-Functional Requirements

- Security: writeback stays limited to one regular file. Network and mount policy are unchanged. Skipping an unchanged file is stricter, not more permissive.
- Performance: one extra read of `auth.json` at startup. The file holds tokens, not a large blob.
- Compatibility: `RunRequest` and the CLI flags are unchanged. Claude writeback stays disabled.

## Constraints

- Technical: do not mount the host `.codex` directory into the sandbox. Do not take `auth.lock` for the whole run; Codex would stall for the length of the agent.
- Timeline: one bugfix, no refactor of the bwrap or Seatbelt plans.

## Success Metrics

- Metric: host `auth.json` contents after an unchanged sandbox copy.
- Baseline: host file is replaced by the startup snapshot.
- Target: host file bytes stay as they were at exit.

## Rollout

- Rollout plan: ship in `skilllite_sandbox::run`. Callers do not change flags.
- Rollback plan: revert the snapshot compare and the `auth.lock` acquisition.
