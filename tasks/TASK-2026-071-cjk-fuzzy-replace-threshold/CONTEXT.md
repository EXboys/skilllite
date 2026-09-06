# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-fs/src/search_replace.rs` (`levenshtein_similarity`,
    `fuzzy_find_similarity`, `apply_replace_fuzzy`)
  - Caller: `crates/skilllite-agent/src/extensions/builtin/file_ops/search_replace.rs`
- Current behavior: `1.0 - levenshtein_distance(a, b) as f64 / a.len().max(b.len()) as f64`
  where `len()` is UTF-8 bytes and distance is character edits.

## Architecture Fit

- Layer boundaries involved: `skilllite-fs` (pure string transform). Agent
  remains a caller.
- Interfaces to preserve: `apply_replace_fuzzy` return type and threshold env.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: CJK fuzzy hits that depended on byte inflation
  stop matching. ASCII behavior unchanged.

## Design Decisions

- Decision: use `chars().count()` as the denominator.
  - Rationale: Levenshtein already counts Unicode scalars; the ratio must use
    the same unit or the 0.85 threshold is language-dependent.
  - Alternatives considered: raise CJK-specific threshold; disable similarity
    fuzzy for non-ASCII.
  - Why rejected: a single consistent metric is smaller and easier to test.

## Open Questions

- [x] Is this the same class as previously deferred "fuzzy first-match"?
      No. That note was about choosing among multiple similar matches. This
      bug makes a *different* CJK line exceed 0.85 when character similarity
      is only 0.80.
