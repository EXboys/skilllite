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

## Checklist

- [ ] Are all task artifact sections written in English?
- [ ] Are non-English source requirements interpreted in English where needed?
- [ ] Are command outputs and code literals preserved verbatim?
