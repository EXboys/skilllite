# Entry Points and Capability Domains

> One-page mental model: what entry points SkillLite has, which crates each depends on, and a one-line use case. Reduces onboarding cost and cognitive load from multiple entry points.

---

## Overview

| Entry | What it is | Crate / component dependencies | Use case (one line) |
|-------|-------------|---------------------------------|----------------------|
| **CLI** | Main binary `skilllite` | core, sandbox, commands, (optional) executor, agent, swarm, artifact HTTP, unified gateway host | Terminal users, scripts, CI: run skills, scan, chat, init, and full feature set. |
| **Python** | python-sdk + IPC/subprocess (+ stdlib HTTP for artifacts) | Calls local `skilllite` binary (`serve` / subcommands); `artifact_put`/`artifact_get` hit artifact HTTP | Python apps: scan_code, execute_code, chat, run_skill; optional cross-process blobs via artifact API. |
| **MCP** | Subcommand `skilllite mcp` | Same as CLI main binary (mcp module lives in skilllite package) | Cursor/VSCode etc.: MCP protocol exposes list_skills, run_skill, scan_code, execute_code. |
| **Desktop** | skilllite-assistant (Tauri) — optional GUI project | Released `skilllite` binary only (`agent-rpc` + CLI `--json`); no engine crate path deps | Desktop users: GUI chat (optional **image attachments** → multimodal `agent_chat`), session management, evolution UI, runtime provisioning, transcript/memory views. |
| **Swarm** | Subcommand `skilllite swarm` | skilllite-swarm (+ main binary; with agent, includes swarm_executor) | Multi-machine / multi-agent: mDNS discovery, P2P task routing, NewSkill sync. |

---

## 1. CLI (main binary `skilllite`)

- **Entry**: `skilllite` (default) or lightweight `skilllite-sandbox` (sandbox + MCP only, no executor/agent).
- **Dependencies**:
  - **Required**: `skilllite-core`, `skilllite-sandbox`, `skilllite-commands`
  - **Optional**: `skilllite-executor` (session/memory), `skilllite-agent` (chat/run agent), `skilllite-swarm` (swarm subcommand)
- **Main subcommands**: `run`, `exec`, `scan`, `validate`, `info`, `security-scan`, `bash`, `serve` (IPC), `gateway serve` (unified HTTP host; **bind** requires `SKILLLITE_GATEWAY_SERVE_ALLOW=1`), `artifact-serve` (standalone artifact HTTP; **bind** requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`), `channel serve` (standalone inbound webhook; **bind** requires `SKILLLITE_CHANNEL_SERVE_ALLOW=1`), `chat`, `init`, `mcp`, `swarm`, `evolution`, `init-cursor`, `dependency-audit`, etc.
- **Use case**: Primary terminal and script entry; CI, automation, local development.

---

## 2. Python (python-sdk + IPC)

- **Entry**: Python package `skilllite` (`python-sdk/`), calling local `skilllite` via **IPC** (`skilllite serve` stdio JSON-RPC) or **subprocess**.
- **Dependencies**: No direct Rust dependency; runtime depends on an installed `skilllite` binary (pip or PATH).
- **Main API**: `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`; `artifact_put` / `artifact_get` (stdlib `urllib`) against the artifact HTTP API (`skilllite gateway serve --artifact-dir ...`, `skilllite artifact-serve`, or any compatible server); IPC in `ipc.py` connects to `serve`, otherwise subprocess.
- **Use case**: Python applications, LangChain/LlamaIndex integration, server or local scripts.

---

## 3. MCP (`skilllite mcp`)

- **Entry**: CLI subcommand `skilllite mcp`, running an MCP (Model Context Protocol) server over stdio.
- **Dependencies**: Same as main binary (skilllite package `mcp/` module + skilllite-commands, etc.); not a separate binary.
- **Exposed tools**: list_skills, get_skill_info, run_skill, scan_code, execute_code, etc.
- **Use case**: Cursor, VSCode, and other MCP-capable IDEs; configure to start `skilllite mcp`.

---

## 4. Desktop (skilllite-assistant)

- **Product role**: **Optional** GUI distribution of the engine — not the default integrator path ([Path 2 — Sandbox & MCP](./START_PATHS.md#path-2-sandbox-mcp) is). Lives in a **separate repository** ([Assistant split architecture](./ASSISTANT-SPLIT-ARCHITECTURE.md)).
- **Entry**: [EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant). Build there with `npm run tauri build`. Engine paths `skilllite-assistant/` and `crates/skilllite-assistant/` are stubs.
- **Integration model** — three layers only:
  - **L1** `skilllite agent-rpc` — streaming chat, confirm/clarify.
  - **L2** `skilllite … --json` — evolution panel, runtime install, skill list.
  - **L3** Workspace files — prompts/transcript paths where UI reads disk directly.
- **Dependencies (D1′)**: **No** path deps on engine crates; semver-pinned **`skilllite` binary** only. Prebuild locates an engine checkout via `SKILLLITE_ENGINE_ROOT` / parent walk, or copies an installed binary.
- **Capabilities**: GUI chat, session management, evolution review/triggering, runtime probing/provisioning, IDE layout, multimodal `agent_chat`.
- **Use case**: Users who want a local app without wiring MCP in an IDE.
- **Layout**: P4 published as [EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant). Engine retain stubs + pointer docs. Export history: `cursor/skilllite-assistant-export-3c6e` / `bash scripts/push-assistant-repo.sh`.

---

## 5. Swarm (`skilllite swarm`)

- **Entry**: CLI subcommand `skilllite swarm --listen <ADDR>`, long-running P2P daemon. Default listen is `127.0.0.1:7700` (loopback). Use `0.0.0.0:<port>` for LAN access; set `SKILLLITE_SWARM_TOKEN` so HTTP clients must send `Authorization: Bearer`.
- **Dependencies**: `skilllite-swarm` (mDNS, mesh, task routing); when used with agent, main binary provides `swarm_executor` to run NodeTasks locally.
- **Capabilities**: Node discovery, task routing, NewSkill Gossip; optionally used with agent features like `skilllite run --soul`.
- **Use case**: Multi-machine collaboration, multi-agent mesh, distributed skill discovery and execution.

---

## Dependency direction (brief)

- **Core** does not depend on upper layers; **sandbox / fs / executor / agent / evolution / commands** depend on core or each other by layer.
- **Main binary** skilllite aggregates commands and optional features; **skilllite-assistant** is an optional GUI that talks to the `skilllite` binary only; **skilllite-swarm** depends only on core and is wired in by the main binary via feature for the `swarm` subcommand.

For detailed crate list and directory layout, see [ARCHITECTURE.md](./ARCHITECTURE.md). 中文版：[入口与能力域一览](../zh/ENTRYPOINTS-AND-DOMAINS.md).
