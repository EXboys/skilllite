# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/builtin/file_ops/grep.rs`
  - `crates/skilllite-agent/src/extensions/builtin/helpers.rs`
  - `crates/skilllite-agent/src/extensions/builtin/file_ops/mod.rs` (`read_file` policy)
  - `crates/skilllite-fs/src/grep.rs`
- Current behavior:
  - `read_file` calls `is_sensitive_read_path` then `filter_sensitive_content_in_text`.
  - `grep_files` only checks workspace containment, then `grep_directory` reads every non-binary file, including `.env`.
  - `grep_directory` skips dot-directories (so `.git/config` is already skipped) but not dot-files.

## Architecture Fit

- Layer boundaries involved: agent builtin tools may apply policy; `skilllite-fs` stays a generic walker with an optional skip callback.
- Interfaces to preserve: `grep_files` JSON schema (`pattern`, `path`, `include`).

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: Agents can no longer grep blocked secret files. That matches the documented `read_file` policy.

## Design Decisions

- Decision: Add an optional skip-file callback to `grep_directory` instead of post-filtering results.
  - Rationale: Post-filter after `max_matches=50` can hide later non-secret hits if `.env` fills the cap.
  - Alternatives considered: Filter after the walk only; raise the cap.
  - Why rejected: Secret files would still be read into memory, and the match budget would remain stealable.
- Decision: Reuse the existing suffix/path helper rather than inventing a grep-only denylist.
  - Rationale: One policy for all file-read tools.
  - Alternatives considered: Skip every dotfile.
  - Why rejected: Would hide `.gitignore` while still missing `secret.key`.

## Open Questions

- [x] Should dotenv variants (`.env.local`) be blocked here? No — tracked by open PR `#143`.
- [x] Should preview_server symlink follow be fixed in the same PR? No — separate blast radius; keep this change minimal.
