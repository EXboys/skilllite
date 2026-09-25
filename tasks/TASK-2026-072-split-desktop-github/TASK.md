# TASK Card

## Metadata

- Task ID: `TASK-2026-072`
- Title: Publish desktop Assistant as a separate GitHub repository
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-09-25`
- Target milestone:

## Problem

User clarification (ZH): 「我意思是单独拆出去一个github项目吧桌面拆出去」.

English: they want SkillLite Assistant on its own GitHub repository (`EXboys/skilllite-assistant`), not only a folder inside `EXboys/skilllite`.

This environment cannot create repositories under `EXboys` (GitHub App token: `createRepository` denied). The work is: make a push-ready standalone history, cut desktop sources out of the engine repo, and document the new URL.

## Scope

- In scope:
  - Standalone README / LICENSE / workflows prepared on the export branch
  - `git subtree split` export branch
  - Engine repo: remove desktop sources, leave pointers to `https://github.com/EXboys/skilllite-assistant`
- Out of scope:
  - Creating the empty GitHub repo (requires org owner / PAT)

## Acceptance Criteria

- [x] Assistant tree is ready to be the root of its own git repo (`origin/cursor/skilllite-assistant-export-3c6e`)
- [x] Export branch exists and is pushed to `origin`
- [x] Engine docs/CI no longer treat desktop as in-tree product source
- [x] Engine root README links the new GitHub project
- [x] `python3 scripts/validate_tasks.py` passes
- [ ] Empty `EXboys/skilllite-assistant` created by owner and `bash scripts/push-assistant-repo.sh` succeeds (blocked on org permission)

## Risks

- Risk: New remote does not exist yet
  - Impact: Cannot `git push` the split until the owner creates the empty repo
  - Mitigation: Keep export on origin (`cursor/skilllite-assistant-export-3c6e`); one-command push script

## Validation Plan

- Required tests: `python3 scripts/validate_tasks.py`; confirm export branch contains assistant root files
- Commands to run: `git ls-tree --name-only origin/cursor/skilllite-assistant-export-3c6e`
- Manual checks: engine README points at the new repo URL

## Regression Scope

- Areas likely affected: engine CI deny/desktop release, contributor start paths
- Explicit non-goals: engine CLI/MCP behavior

## Links

- Source TODO section: user follow-up
- Related PRs/issues: #158 (in-tree extract)
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`
