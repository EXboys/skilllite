# PRD

## Background

Daily high-severity sweep on `main` @ `6d5c5b9`. Fuzzy `search_replace` is the
agent fallback when `old_string` is not an exact substring. The similarity
denominator used byte length, which inflates CJK scores.

## Objective

- Fuzzy similarity uses Unicode scalar counts so the 0.85 threshold means the
  same thing for ASCII and CJK.
- A two-character name swap in a typical Chinese sentence no longer overwrites
  the wrong line.

## Functional Requirements

- FR-1: `levenshtein_similarity` divides character edit distance by
  `max(a.chars().count(), b.chars().count())`.
- FR-2: Default threshold 0.85 is unchanged (`SKILLLITE_FUZZY_THRESHOLD` still applies).
- FR-3: Exact match, whitespace fuzzy, and blank-line fuzzy are unchanged.

## Non-Functional Requirements

- Security: no policy change.
- Performance: `chars().count()` is O(n) on already-trimmed lines; negligible.
- Compatibility: CJK fuzzy matches that only passed because of byte inflation
  will now fail closed (agent must copy closer text). That is the intended
  correctness fix.

## Constraints

- Technical: minimal change in `crates/skilllite-fs/src/search_replace.rs`.
- Timeline: same sweep PR.

## Success Metrics

- Metric: CJK 2-of-10-character difference does not fuzzy-replace at 0.85.
- Baseline: current code reports ~0.933 and replaces.
- Target: similarity 0.80; `apply_replace_fuzzy` returns not-found.

## Rollout

- Rollout plan: merge with other mainline fixes.
- Rollback plan: revert the one-line denominator change.
