# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-artifact/src/local_dir.rs`. Callers: `run_artifact_http_server` and `skilllite/src/dispatch/gateway.rs` (`artifact_router` on `LocalDirArtifactStore`).
- Current behavior: `atomic_write_bytes` uses `path.with_extension("tmp")`. For `notes.json` that path is `notes.tmp`, which `validate_artifact_key` accepts. `fs::write` replaces that file and `fs::rename` moves it onto `notes.json`, so the previous key is gone. `archive.tar.gz` collides with `archive.tar.tmp`. A key with no extension, `report`, collides with `report.tmp`. Concurrent puts of `notes.json` and `notes.md` share `notes.tmp`.

## Architecture Fit

- Layer boundaries involved: `skilllite-artifact` local backend only. `skilllite-core` validation is unchanged.
- Interfaces to preserve: `ArtifactStore::put` upsert semantics, key rules, and HTTP status mapping.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: Successful keys stay at the same paths. A put no longer removes a different key whose path happens to equal the old temp path.

## Design Decisions

- Decision: Write to `.skilllite-partial-<pid>-<nanos>-<seq>` in the destination directory, then rename onto the key path. Delete that temp file if write or rename fails.
  - Rationale: The temp name is not derived from the key stem, so it is not another key's `with_extension("tmp")` path. Pid plus an atomic counter keeps concurrent puts in one process from sharing a temp file. Rename stays on the same directory.
  - Alternatives considered: `path.with_extension(format!("tmp.{}", pid))`, as scan-cache does. A dedicated temp directory.
  - Why rejected: `notes.json` would still use `notes.tmp.<pid>`, and a stored key of that name would be deleted. A temp directory whose name is a legal key prefix can be listed or written as an artifact key. A hidden sibling file avoids both.

## Open Questions

- [x] `skilllite_fs::atomic_write` has the same `with_extension("tmp")` shape for checkpoints and evolution JSON. It is out of scope until a default caller shows a second file sharing that temp path.
