# Security Non-Negotiables

Scope: any change in `skilllite-sandbox`, security scanning, dependency audit, or execution gating.

## Must

- Keep default security level semantics intact:
  L1 (no sandbox), L2 (isolation), L3 (isolation + static scan).
- Preserve two-phase confirmation for high-risk execution (scan first, execute after approval).
- Keep Linux behavior fail-closed: reject by default when no valid isolation backend is available (except explicitly enabled controlled fallback).
- Keep critical security events auditable (at minimum: trigger reason, policy branch, result).
- Add tests or regression coverage for any path/network/process policy change.

## Must Not

- Do not enable auto-approval of dangerous operations by default.
- Do not silently relax network/filesystem/process restrictions.
- Do not remove or bypass integrity checks (hash/tamper detection) without an equivalent replacement.
- Do not change default deny rules without explicit impact analysis.

## Required Review Notes (PR body)

- [ ] What security policy changed, and why is it needed?
- [ ] Is default behavior more permissive? If yes, what are the risks and compensating controls?
- [ ] Does this affect `SKILLLITE_*` config semantics or backward compatibility?
- [ ] Were tests and EN/ZH docs updated?

## Quick Verify

- `cargo test -p skilllite-sandbox`
- `cargo test -p skilllite`
- `cargo audit`
