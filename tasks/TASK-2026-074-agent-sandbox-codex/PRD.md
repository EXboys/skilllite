# PRD

## Background

The subscription sandbox already isolates one agent process. It was wired only to Claude Code: the login directory is `.claude`, and the default command is `claude -p`. Codex CLI stores a ChatGPT session in `.codex/auth.json` and refreshes that file during a run.

## Objective

`skilllite agent-sandbox --runtime codex` starts Codex from a `.codex` directory under the same bwrap or Seatbelt profile, and keeps a refreshed `auth.json`.

## Functional Requirements

- FR-1: `runtime` is `claude` or `codex`. The default is `claude`.
- FR-2: Claude copies `.claude` and does not write it back. Codex copies `.codex`.
- FR-3: An empty `argv` uses the runtime default. Codex's default disables Codex's own sandbox because the outer sandbox is the boundary.
- FR-4: After the Codex process exits, a regular `auth.json` is copied back. A missing file is a no-op. A symlink is an error and is not followed.
- FR-5: Stdout and the stderr excerpt redact tokens that start with `sk-`.

## Non-Functional Requirements

- Security: Outer write roots and the proxy allowlist stay as they are. The Codex bypass flag does not open the host home or drop the proxy pin.
- Performance: No extra process beyond the existing run.
- Compatibility: Callers that omit `runtime` still run Claude.

## Constraints

- Technical: Login still happens on the host (`claude auth login` or `codex login`). The program does not read the macOS keychain.
- Timeline: Sandbox and CLI only. The pool is unchanged.

## Success Metrics

- Metric: A Codex-shaped run persists `auth.json` and leaves `config.toml` untouched.
- Baseline: Only `.claude` could be copied, and nothing was written back.

## Rollout

- Rollout plan: Ship behind `--runtime`, default `claude`.
- Rollback plan: Omit the flag. Existing Claude calls keep the previous command.

## Non-Goals

- Translating Codex JSONL into Claude `stream-json`.
- A subscription pool of Codex accounts.
