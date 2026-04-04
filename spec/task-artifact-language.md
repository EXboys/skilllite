# Task Artifact Language Policy

Scope: task execution artifacts under `tasks/TASK-.../` (`TASK.md`, `PRD.md`, `CONTEXT.md`, `STATUS.md`, `REVIEW.md`) and task board entries.

## Must

- Write task artifacts in English by default.
- Keep section titles, checklist labels, and status values in English.
- If the source requirement is non-English, include a brief English interpretation in the artifact.
- Keep links, commands, file paths, and code identifiers unchanged.

## Must Not

- Do not write new task artifacts primarily in non-English languages.
- Do not mix multiple languages in one section unless quoting user/source text.
- Do not translate literal code, command output, or API field names.

## Allowed Exceptions

- Direct quotes from user requests or external sources may remain in the original language.
- Proper nouns and product names can remain untranslated.

## Mechanical shape (CI)

`python3 scripts/validate_tasks.py` enforces file presence and **fixed headings / substrings** on every `tasks/TASK-*/` directory. Authors and agents **must** keep these intact (do not replace with a single-line journal that drops the headers):

- **`STATUS.md`**: must contain the Markdown headings exactly `## Timeline` and `## Checkpoints` (see `tasks/_templates/STATUS.md`).
- **`REVIEW.md`**: must contain the substring `Merge readiness:` (e.g. under `## Decision` as `- Merge readiness: ready | not ready` — see `tasks/_templates/REVIEW.md`).

Before opening a PR that touches `tasks/TASK-.../`, run:

```bash
python3 scripts/validate_tasks.py
```

## Checklist

- [ ] Are all task artifact sections written in English?
- [ ] Are non-English source requirements interpreted in English where needed?
- [ ] Are command outputs and code literals preserved verbatim?
- [ ] Does `STATUS.md` still include `## Timeline` and `## Checkpoints`?
- [ ] Does `REVIEW.md` still include `Merge readiness:`?
- [ ] Did you run `python3 scripts/validate_tasks.py` when task files changed?
