# Docs Sync Policy (EN/ZH)

Scope: any change that affects user, contributor, or integrator understanding.

## Must Update Docs When

- CLI commands, flags, defaults, or output semantics change.
- Environment variables are added/renamed/deprecated, or defaults change.
- Architecture boundaries, crate relationships, or entry capability domains change.
- Security policy, execution gating, or risk wording changes.
- Installation/release/platform support matrix changes.

## Required Files (choose by change scope)

- User-visible behavior: `README.md` and `docs/zh/README.md`
- Quick start/commands: `docs/en/GETTING_STARTED.md` and `docs/zh/GETTING_STARTED.md`
- Architecture and boundaries: `docs/en/ARCHITECTURE.md` and `docs/zh/ARCHITECTURE.md`
- Entry points and capability domains: `docs/en/ENTRYPOINTS-AND-DOMAINS.md` and `docs/zh/ENTRYPOINTS-AND-DOMAINS.md`
- Environment variables: `docs/en/ENV_REFERENCE.md` and `docs/zh/ENV_REFERENCE.md`

## PR Checklist

- [ ] Is there any behavior drift where code changed but docs did not?
- [ ] Were EN and ZH both updated with consistent meaning?
- [ ] Are example commands runnable (flags/subcommands/paths correct)?
- [ ] Is compatibility/migration documented for breaking changes?

## Quality Bar

- Document current behavior only; do not present future plans as shipped behavior.
- Keep terminology consistent across README and docs.
