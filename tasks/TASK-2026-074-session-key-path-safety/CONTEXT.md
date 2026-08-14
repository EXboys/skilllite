# CONTEXT

## Technical Boundaries

- Shared validation lives in `skilllite-core::path_validation::{validate_session_key, session_file_under_dir}`.
- Filesystem joins for transcripts/plans live in `skilllite-executor::{transcript, plan}`.
- Agent CLI/RPC and schedule tick validate early; path builders remain the last line of defense.
- Desktop assistant transcript listing uses a local equivalent check (crate does not depend on `skilllite-core`).

## Root Cause

`Path::join` replaces the base path when the joined component is absolute. Transcript/plan helpers built filenames as `{session_key}-YYYY-MM-DD.jsonl` and joined under `transcripts/` / `plans/` without validating `session_key`, so values like `/tmp/evil` wrote outside the chat root. Relative `../` keys likewise escaped after OS path resolution.

## Compatibility Notes

- Valid single-segment keys including Unicode remain accepted.
- No schema/version migration required.
