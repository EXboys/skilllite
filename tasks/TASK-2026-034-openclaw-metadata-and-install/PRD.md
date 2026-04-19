# PRD

## Background

SkillLite already ingests SKILL.md authored against the Claude Agent Skills
specification. OpenClaw / ClawHub extends that base with an additional
`metadata.openclaw.*` (and `clawdbot` / `clawdis` aliases) block whose most
operationally important members are `requires.*` and the declarative
`install[]` spec. SkillLite previously merged only `requires.bins` and
`requires.env`, and it ignored `install[]` entirely, so OpenClaw skills could
not feed SkillLite's dependency / environment build pipeline without manual
duplication into the free-form `compatibility` string.

## Objective

Make OpenClaw / ClawHub `SKILL.md` skills work with SkillLite's existing
package install pipeline (npm + pip via `env::builder`) without requiring
users to mirror declarations into `compatibility` text.

## Functional Requirements

- FR-1: Parse OpenClaw `metadata` (with `clawdbot` / `clawdis` aliases) and
  surface `requires.bins / anyBins / env / config`, `primaryEnv`, `os`,
  `skillKey`, `always` and an `install[]` summary inside the existing
  `compatibility` string.
- FR-2: Expose structured install entries on `SkillMetadata.openclaw_installs`
  with classification by kind: `node` -> npm packages, `uv` -> pip packages,
  `brew` / `go` -> system bin names (recorded only), unknown -> recorded with
  warning.
- FR-3: `deps::detect_dependencies` consumes the structured installs as a
  third-priority signal (after lock + compatibility whitelist) and returns the
  appropriate `DependencyType`.
- FR-4: `evolution::env_helper::ensure_skill_deps_and_env` falls back to the
  structured installs when the existing resolver path produced nothing.

## Non-Functional Requirements

- Security: Do **not** invoke `brew` or `go install` automatically — both
  modify host state outside the SkillLite sandbox. Only log the declaration.
- Performance: Parsing is YAML-only and runs at SKILL.md load time; no
  network calls are added.
- Compatibility: Pre-existing SKILL.md files without `metadata.openclaw.*`
  must keep their original `compatibility` value byte-for-byte.

## Constraints

- Technical: `serde_json::Value` is the on-the-wire form already produced by
  the existing YAML parser; the new module reads that without re-deserialising.
- Timeline: One iteration; no migration of `.skilllite.lock` required.

## Success Metrics

- Metric: Tests added under `skill::openclaw_metadata::tests` and
  `skill::deps::tests` cover the new branches.
- Baseline: 6 OpenClaw-related tests before this task.
- Target: 10+ green tests covering aliases, install summary, structured
  classification, and deps fallback (achieved: 12 OpenClaw-related tests
  now pass).

## Rollout

- Rollout plan: Ship in next minor release; behavior change is
  additive — new compatibility text fragments and a new structural fallback
  for empty package lists.
- Rollback plan: Revert the `openclaw_metadata` module wiring and the
  `SkillMetadata.openclaw_installs` field; existing call sites already
  treat the field as optional.
