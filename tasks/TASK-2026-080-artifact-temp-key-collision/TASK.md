# TASK Card

## Metadata

- Task ID: `TASK-2026-080`
- Title: Keep artifact temp files out of the key namespace
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-24`
- Target milestone:

## Problem

`LocalDirArtifactStore::put` writes bytes to `path.with_extension("tmp")` and then renames that path onto the destination. `Path::with_extension` replaces the last extension, so the temp path of `notes.json` is the stored key `notes.tmp`. A successful put of `notes.json` deletes an existing `notes.tmp` artifact and returns success. The same temp path is shared by `notes.md`, `archive.tar.gz` (temp `archive.tar.tmp`), and a key with no extension (`report` uses `report.tmp`). Two overlapping puts of those keys can also publish one key's bytes as the other.

`artifact-serve` and `gateway serve` both call this put from concurrent HTTP handlers.

## Scope

- In scope:
  - Write the temp file under a name that is not `with_extension("tmp")`.
  - Remove the temp file if the write or rename fails.
  - Regression test: putting `notes.json` keeps an existing `notes.tmp`, including non-ASCII bytes.
- Out of scope:
  - `skilllite_fs::atomic_write` (same helper shape, different call sites; not this store).
  - Scan-cache temp names, which already include the process id and are a single file.
  - Windows `rename` over an existing file (already the behavior of this put).

## Acceptance Criteria

- [x] Putting `notes.json` after `notes.tmp` leaves both keys intact, including a non-ASCII body.
- [x] Putting `archive.tar.gz` does not remove `archive.tar.tmp`. Putting `report` does not remove `report.tmp`.
- [x] A failed rename does not leave the new temp name behind. The success path does not leave a `.skilllite-partial-*` file.
- [x] `cargo test -p skilllite-artifact` passes.

## Risks

- Risk: A temp name that is itself a legal artifact key could still be overwritten if a caller stores that exact name during the write.
  - Impact: One artifact key could be replaced by another put's temp bytes.
  - Mitigation: The name is a hidden `.skilllite-partial-<pid>-<nanos>-<seq>` file, not a stem derived from the key. The regression test locks the previous collision.

## Validation Plan

- Required tests: `cargo test -p skilllite-artifact`, `cargo fmt --check`, `cargo clippy -p skilllite-artifact --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: none beyond the unit test. The failure mode is deterministic without a live HTTP server.

## Regression Scope

- Areas likely affected: `LocalDirArtifactStore::put`, and therefore `artifact-serve` and gateway artifact routes
- Explicit non-goals: artifact auth, key validation rules, HTTP status codes

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `docs/openapi/artifact-store-http-v1.yaml` (key rules unchanged)
