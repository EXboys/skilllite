# CONTEXT

## Technical Background

- Pending skills live at `skills_root/_evolved/_pending/<name>/`.
- `generate_skill_inner` builds `skill_dir = pending_dir.join(parsed.name)` and `script_path = skill_dir.join(parsed.entry_point)`, then writes via `skilllite_fs::write_file`.
- `gatekeeper_l1_path` checks `skill_dir` only (lexical `starts_with` under `_evolved`), not the resolved script path.
- Lexical `Path::starts_with` after joining `../` still reports containment, so rejecting `Component::ParentDir` (and absolute/drive/`\\`) is required.

## Constraints

- Compatibility: keep multi-segment relative entry points (`scripts/main.py`).
- Do not depend on unmerged PR #89 helpers; add local validators in `skill_synth`.
- Follow crate `Error::validation` / `bail!` conventions (no unwrap in production paths).

## Surrounding System Impact

- `generate.rs`, `refine.rs`, and `repair.rs` write scripts using entry_point joins.
- Infer/test invoke also joins entry_point for execution; validate before join to avoid running escaped paths.

## Compatibility Notes

- Existing valid pending skills with normal names and relative entry points unchanged.
- Malicious/malformed model outputs that previously escaped now skip or fail closed.
