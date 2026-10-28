# PRD

## Background

The public SkillLite repository provides the sandbox primitive. Pool, binding, and the phone SSE live in a private repository and are not part of this task.

## Objective

`skilllite_sandbox::run` starts one sandboxed process for a subscription directory and streams redacted stdout lines to the caller.

## Functional Requirements

- FR-1: Require `account_dir` (with `.claude`), `work_dir`, `prompt`, and `https_proxy`.
- FR-2: Copy `.claude` into an ephemeral home. Do not mount or write back the original directory.
- FR-3: Mount `work_dir` in place (`/work` on Linux, the process working directory on macOS).
- FR-4: One exclusive lock per subscription directory.
- FR-5: `require_proxy` defaults to false. When it is true, a missing `https_proxy` does not start. When a proxy is set, resolve it on the host: Linux allows only those IP addresses and that port from the run cgroup, and macOS allows loopback on that port and rejects a non-loopback proxy. Do not resolve DNS inside the sandbox.
- FR-6: Default argv is `claude -p <prompt> --output-format stream-json --verbose --permission-mode dontAsk`.
- FR-7: Replace `sk-ant-…` tokens with `[redacted]` before the line callback.
- FR-8: On exit, cancel, or error, kill the process group and delete the ephemeral home.

## Non-Functional Requirements

- Security: skill `execute()` policy is unchanged. This path does not read, store, or forward credential file contents.
- Performance: stream lines as they arrive.
- Compatibility: macOS and Linux. Windows returns a validation error.

## Constraints

- Technical: no dependency on Agent Doctor or Evotown. Linux network allowlisting is fail-closed when nftables or a private cgroup is unavailable.
- Timeline: this task only.

## Success Metrics

- Metric: sandbox unit tests cover rejection, redaction, lock, cleanup, and the Linux argv plan.
- Baseline: no `run` API.
- Target: `cargo test -p skilllite-sandbox` passes.

## Rollout

- Rollout plan: library API plus `agent-sandbox` CLI.
- Rollback plan: remove the module and the CLI variant. Skill execution does not call it.
