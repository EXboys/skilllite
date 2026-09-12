# PRD

## Summary

Prevent Windows absolute and rooted paths from escaping the run-scoped artifact store root.

## User / Security Impact

Operators who enable `artifact-serve` or `gateway serve --artifact-dir` on Windows (or any caller of `LocalDirArtifactStore`) must not be able to read or write files outside `<base>/artifacts` via crafted `run_id` or `key` values.

## Requirements

1. Artifact keys must remain relative paths using `/` separators only.
2. Run IDs must be single path segments without separators or drive prefixes.
3. Store implementations must fail closed if a constructed path would leave the artifacts root.
4. Validation behavior must be identical on Linux/macOS/Windows hosts so malicious Windows paths cannot be staged from other platforms.

## Non-Goals

- Changing HTTP routes, body limits, or bearer-token defaults.
- Fixing symlink follow-out after a legitimate in-root path.
