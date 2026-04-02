# Structured Signal First (No Regex-First in Core)

Scope: agent / commands / mcp behavior tasks, especially completion/outcome classification and evolution triggers.

## Core Principle

For generic platform capabilities, use **LLM semantic judgment + structured runtime facts** as the primary decision path.
Regex/text phrase matching is allowed only as a constrained fallback, never the main decision path.

## Must

- Design outcome/completion classification from structured signals first (typed fields, execution state, tool success/failure facts, planner state).
- Prefer LLM-based semantic classification for complex, open-domain user scenarios; then validate against structured execution facts.
- Keep declarations consistent with execution facts (for example, `completion_type=success` must not contradict failure/replan evidence).
- If fallback heuristics are unavoidable, keep them narrow, auditable, and explicitly lower-priority than structured signals.
- Add tests proving structured-path decisions work without relying on regex phrases.
- Document fallback trigger conditions and expected downgrade behavior (`success -> partial_success/failure`) when conflicts are detected.

## Must Not

- Do NOT use regex phrase matching as the primary classifier for generic core behavior.
- Do NOT replace LLM semantic classification with phrase lists for open-domain intent/outcome judgment.
- Do NOT treat natural-language wording as stronger evidence than execution/runtime facts.
- Do NOT silently default ambiguous completion to `success`.

## Checklist

- [ ] Is the primary decision path based on structured runtime signals?
- [ ] For complex/open-domain scenarios, is LLM semantic judgment used as the primary classifier?
- [ ] If regex/text heuristics exist, are they fallback-only and explicitly lower-priority?
- [ ] Do tests cover contradiction cases between declaration and execution facts?
- [ ] Is downgrade/rejection behavior defined when declaration conflicts with facts?
