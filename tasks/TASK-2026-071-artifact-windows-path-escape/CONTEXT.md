# CONTEXT

## Technical Boundaries

- Validation lives in `skilllite-core::artifact_store::validate_artifact_key`.
- Run ID validation lives in `skilllite-artifact::validation::validate_run_id`.
- Filesystem join happens in `LocalDirArtifactStore::artifact_path`.
- HTTP handlers call the same validators before store operations.

## Root Cause

1. `validate_artifact_key` rejects leading `/` or `\` and `..`, but accepts `C:\...` / `C:/...` because they do not start with a separator.
2. `validate_run_id` rejects `/` and `..` but not `\`, so `\Windows\Temp` is accepted.
3. On Windows, `PathBuf::join` with an absolute component discards the previous base path.

## Constraints

- Keep hierarchical keys with `/` (e.g. `step1/output.json`).
- Prefer platform-independent string checks plus a lexical containment guard after join.
- Avoid requiring Windows CI to prove drive-letter rejection; validators must reject those strings on Linux too.

## Compatibility

- Previously accepted keys containing `\` or drive prefixes become invalid. This is an intentional security break for malformed/hostile inputs only.
