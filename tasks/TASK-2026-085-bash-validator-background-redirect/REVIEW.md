# REVIEW

## Findings

- Root cause confirmed: `CHAIN_OPERATORS` omitted bare `&`, `>`, `<` while execution uses unsandboxed `sh -c`.
- Fix is minimal and fail-closed; no API surface change.
- False-positive risk for URL query `&` is accepted and documented (same substring policy as `;` / `|`).
- Falsifiability: without the new operators, the five injection payloads remain ACCEPT; with them they BLOCK; valid `agent-browser open https://example.com` stays ACCEPT.

## Security review notes

- [x] What security policy changed, and why is it needed? — Expanded chain/redirect operator deny list to close host RCE/file-write via bash-tool skills.
- [x] Is default behavior more permissive? — No; stricter only.
- [x] Does this affect `SKILLLITE_*` config semantics or backward compatibility? — No env semantics; previously-accepted unsafe command strings are now rejected.
- [x] Were tests and EN/ZH docs updated? — Yes.

## Merge readiness: ready to merge after PR CI
