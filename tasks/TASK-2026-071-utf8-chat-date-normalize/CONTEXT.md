# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-agent/src/extensions/builtin/chat_data.rs`, dispatch via `execute_builtin_tool` in `crates/skilllite-agent/src/extensions/builtin/mod.rs`.
- Current behavior: `normalize_date` strips `-` and, when `s.len() == 8`, formats `&s[0..4]-&s[4..6]-&s[6..8]`. `len()` is bytes. `2026年9` is 8 bytes (`2026` + 3-byte `年` + `9`); index 4 is mid-character.

## Architecture Fit

- Layer boundaries involved: `skilllite-agent` builtin tools only.
- Interfaces to preserve: `chat_history` / `chat_plan` JSON arguments and return strings.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: only the 8-ASCII-digit compact form was a valid `YYYYMMDD` key. Non-digit 8-byte strings previously crashed; they now pass through and miss the file.

## Design Decisions

- Decision: require `s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit())` before slicing.
  - Rationale: the compact form is defined as `YYYYMMDD`. ASCII digits are single-byte, so the existing slices are then safe. Matches `spec/rust-conventions.md` (byte slice only when ASCII by construction).
  - Alternatives considered: `chars().take` reconstruction, or a full date parser.
  - Why rejected: char-take would still accept non-dates; a parser is out of scope.

## Open Questions

- [x] Should docs change? No; the tool schema already documents `YYYY-MM-DD` or `YYYYMMDD`.
- [x] Are architecture boundaries affected? No.
