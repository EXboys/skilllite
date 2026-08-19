# PRD

## Background

Agent file tools treat `.env`, `.key`, `.pem`, and `.git/config` as always-blocked reads. `grep_files` is advertised as a content search tool and is routinely used with `path: "."`. On current `main` it reads those files and returns raw matching lines.

## Objective

Close the `grep_files` bypass of the existing sensitive-read policy so credentials in blocked files cannot reach the model context through search.

## Functional Requirements

- FR-1: `grep_files` with a sensitive target path fails closed with the same blocked wording as `read_file`.
- FR-2: Directory greps omit sensitive files entirely (they must not fill the 50-match cap).
- FR-3: Match lines from ordinary files still pass through `filter_sensitive_content_in_text`.
- FR-4: Non-sensitive grep behavior is unchanged.

## Non-Functional Requirements

- Security: Fail closed; do not return secret bytes in the tool result.
- Performance: Skip sensitive files before reading them.
- Compatibility: Same tool name and argument schema.

## Constraints

- Technical: Reuse `is_sensitive_read_path` and `filter_sensitive_content_in_text`. Do not broaden dotenv-variant matching in this change (`#143`).
- Timeline: Same-day high-severity fix.

## Success Metrics

- Metric: Dedicated tests fail if the skip or redaction is removed.
- Baseline: `grep_files {"pattern":"API_KEY"}` leaked `.env` contents.
- Target: The same call returns no secret material.

## Rollout

- Rollout plan: Merge the focused agent-tool fix.
- Rollback plan: Revert the PR; tool schema is unchanged.
