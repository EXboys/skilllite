# PRD

## Summary

Prevent chat session keys from escaping the chat data root when building transcript and plan file paths.

## User / Operator Impact

- Safe default and generated session keys continue to work.
- Malicious or malformed session keys that look like filesystem paths are rejected with a validation error instead of writing/reading outside `chat/transcripts` or `chat/plans`.

## Requirements

1. Session keys used in path joins MUST be a single normal path segment.
2. Absolute paths, `..`, separators, null bytes, and Windows drive prefixes MUST be rejected.
3. Path builders for transcripts and plans MUST validate before `Path::join`.
4. Entry points that accept external session keys (CLI chat/clear-session, agent-rpc, schedule, executor RPC) MUST fail closed on invalid keys.

## Non-Goals

- Redesigning session identity or migrating historical unsafe keys.
- Changing transcript date-segmentation filename format for valid keys.
