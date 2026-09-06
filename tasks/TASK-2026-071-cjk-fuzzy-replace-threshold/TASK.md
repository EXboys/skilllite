# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Use character length for fuzzy search_replace similarity
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors: `agent`
- Created: `2026-09-06`
- Target milestone: next merge

## Problem

`skilllite-fs` fuzzy `search_replace` scores Levenshtein similarity as
`1 - char_edit_distance / byte_len`. For CJK text, each character is typically
3 UTF-8 bytes, so the advertised 0.85 threshold is effectively ~0.55 in
character space. An agent that slightly misquotes a Chinese line can silently
overwrite a different line (for example `请确认用户张三的订单` vs
`请确认用户李四的订单`).

## Scope

- In scope: fix `levenshtein_similarity` to use character counts; add
  regression tests for CJK and ASCII threshold behavior.
- Out of scope: changing the default 0.85 threshold; rewriting fuzzy
  whitespace / blank-line matchers; path-containment or recovered-write work.

## Acceptance Criteria

- [x] CJK lines that differ by two characters in a 10-character line do not
      fuzzy-match at the default 0.85 threshold.
- [x] ASCII similarity (already byte==char) is unchanged.
- [x] Exact CJK `search_replace` still succeeds.
- [x] `cargo test -p skilllite-fs` and workspace `cargo test` / `clippy` / `fmt` pass.

## Risks

- Risk: slightly fewer fuzzy matches on CJK (intended; those matches were false positives).
  - Impact: agents may need a closer `old_string` instead of replacing the wrong line.
  - Mitigation: exact match and whitespace/blank-line fuzzy paths are unchanged.

## Validation Plan

- Required tests: unit tests on `apply_replace_fuzzy` for CJK miss / ASCII control / exact hit.
- Commands to run:
  - `cargo test -p skilllite-fs`
  - `cargo test`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
- Manual checks: none (pure function).

## Regression Scope

- Areas likely affected: agent `search_replace` / `preview_edit` fuzzy fallback;
  failure-hint similarity ranking (same helper).
- Explicit non-goals: exact match, `replace_all`, `normalize_whitespace`.

## Links

- Source TODO section: daily critical-bug sweep 2026-09-06
- Related PRs/issues: none (not covered by #89/#112–#151)
- Related docs: none
