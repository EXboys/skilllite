# Entry Points and Capability Domains

> One-page mental model: what entry points SkillLite has, which crates each depends on, and a one-line use case. Reduces onboarding cost and cognitive load from multiple entry points.

---

## Overview

| Entry | What it is | Crate / component dependencies | Use case (one line) |
|-------|-------------|---------------------------------|----------------------|
| **CLI** | Main binary `skilllite` | core, sandbox, commands, (optional) executor, agent, swarm | Terminal users, scripts, CI: run skills, scan, chat, init, and full feature set. |
| **Python** | python-sdk + IPC/subprocess (+ stdlib HTTP for artifacts) | Calls local `skilllite` binary (`serve` / subcommands); `artifact_put`/`artifact_get` hit artifact HTTP | Python apps: scan_code, execute_code, chat, run_skill; optional cross-process blobs via artifact API. |
| **MCP** | Subcommand `skilllite mcp` | Same as CLI main binary (mcp module lives in skilllite package) | Cursor/VSCode etc.: MCP protocol exposes list_skills, run_skill, scan_code, execute_code. |
| **Desktop** | skilllite-assistant (Tauri) — first-class entry | core, fs, sandbox, agent, evolution (direct path deps); optional runtime fallback to installed `skilllite` for some commands | Desktop users: GUI chat (optional **image attachments** → multimodal `agent_chat`), session management, evolution UI, runtime provisioning, transcript/memory views. |
| **Swarm** | Subcommand `skilllite swarm` | skilllite-swarm (+ main binary; with agent, includes swarm_executor) | Multi-machine / multi-agent: mDNS discovery, P2P task routing, NewSkill sync. |

---

## 1. CLI (main binary `skilllite`)

- **Entry**: `skilllite` (default) or lightweight `skilllite-sandbox` (sandbox + MCP only, no executor/agent).
- **Dependencies**:
  - **Required**: `skilllite-core`, `skilllite-sandbox`, `skilllite-commands`
  - **Optional**: `skilllite-executor` (session/memory), `skilllite-agent` (chat/run agent), `skilllite-swarm` (swarm subcommand)
- **Main subcommands**: `run`, `exec`, `scan`, `validate`, `info`, `security-scan`, `bash`, `serve` (IPC), `artifact-serve` (local artifact HTTP; **bind** requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`), `chat`, `init`, `mcp`, `swarm`, `evolution`, `init-cursor`, `dependency-audit`, etc.
- **Use case**: Primary terminal and script entry; CI, automation, local development.

---

## 2. Python (python-sdk + IPC)

- **Entry**: Python package `skilllite` (`python-sdk/`), calling local `skilllite` via **IPC** (`skilllite serve` stdio JSON-RPC) or **subprocess**.
- **Dependencies**: No direct Rust dependency; runtime depends on an installed `skilllite` binary (pip or PATH).
- **Main API**: `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`; `artifact_put` / `artifact_get` (stdlib `urllib`) against the artifact HTTP API (`skilllite artifact-serve` or any compatible server); IPC in `ipc.py` connects to `serve`, otherwise subprocess.
- **Use case**: Python applications, LangChain/LlamaIndex integration, server or local scripts.

---

## 3. MCP (`skilllite mcp`)

- **Entry**: CLI subcommand `skilllite mcp`, running an MCP (Model Context Protocol) server over stdio.
- **Dependencies**: Same as main binary (skilllite package `mcp/` module + skilllite-commands, etc.); not a separate binary.
- **Exposed tools**: list_skills, get_skill_info, run_skill, scan_code, execute_code, etc.
- **Use case**: Cursor, VSCode, and other MCP-capable IDEs; configure to start `skilllite mcp`.

---

## 4. Desktop (skilllite-assistant)

- **Entry status**: First-class entry (Phase 0 D1, 2026-04-20). Architectural rules, dependency policy (`deny.toml`), CI checks (`cargo deny check bans` runs against this manifest in addition to the root workspace), documentation, and testing strategy treat Desktop on equal footing with the CLI. Desktop is **not** a thin shell over the installed binary.
- **Entry**: Tauri app `skilllite-assistant`. Built via a separate Cargo manifest (`crates/skilllite-assistant/src-tauri/Cargo.toml`) and excluded from the root workspace because Tauri requires platform GUI toolchains (e.g. glib/GTK on Linux). Build with `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` or `npm run tauri build` from `crates/skilllite-assistant/`.
- **Dependencies**:
  - **Build time (direct path deps)**: `skilllite-core`, `skilllite-fs`, `skilllite-sandbox`, `skilllite-agent`, `skilllite-evolution`.
  - **Runtime fallback**: For a subset of commands the bridge still spawns the installed `skilllite` binary (e.g. `agent-rpc` subprocess); see `crates/skilllite-assistant/README.md`. This is a runtime convenience, not a build-time requirement.
- **Capabilities**: GUI chat, session management, evolution review/triggering, runtime probing/provisioning, transcript/memory/output views, IDE three-pane layout, image attachments to multimodal `agent_chat`.
- **Chat input**: `Enter` sends; `Shift+Enter` inserts a newline; during an active IME composition, `Enter` is left to the IME (confirm candidates) and does not send.
- **Use case**: Desktop users who prefer not to use the CLI; tray and shortcut scenarios.
- **Boundary policy**: Direct dependencies on `skilllite-{agent,sandbox,evolution}` are explicitly allow-listed in `deny.toml` for now. Phase 1+ progressively moves shared flows behind a `skilllite-services` crate; existing direct deps remain permissible during migration. Reverting Desktop to a "shell" status would require explicitly overturning D1.

---

## 5. Swarm (`skilllite swarm`)

- **Entry**: CLI subcommand `skilllite swarm --listen <ADDR>`, long-running P2P daemon. Default listen is `127.0.0.1:7700` (loopback). Use `0.0.0.0:<port>` for LAN access; set `SKILLLITE_SWARM_TOKEN` so HTTP clients must send `Authorization: Bearer`.
- **Dependencies**: `skilllite-swarm` (mDNS, mesh, task routing); when used with agent, main binary provides `swarm_executor` to run NodeTasks locally.
- **Capabilities**: Node discovery, task routing, NewSkill Gossip; optionally used with agent features like `skilllite run --soul`.
- **Use case**: Multi-machine collaboration, multi-agent mesh, distributed skill discovery and execution.

---

## Dependency direction (brief)

- **Core** does not depend on upper layers; **sandbox / fs / executor / agent / evolution / commands** depend on core or each other by layer.
- **Main binary** skilllite aggregates commands and optional features; **skilllite-assistant** is a first-class entry that directly consumes `core`, `fs`, `sandbox`, `agent`, and `evolution` (allow-listed in `deny.toml`); **skilllite-swarm** depends only on core and is wired in by the main binary via feature for the `swarm` subcommand.
- The future **`skilllite-services`** crate (Phase 1+) will sit between entry crates (`skilllite`, `skilllite-assistant`, future MCP entry) and domain crates; both CLI and Desktop will progressively migrate shared flows there.

For detailed crate list and directory layout, see [ARCHITECTURE.md](./ARCHITECTURE.md). 中文版：[入口与能力域一览](../zh/ENTRYPOINTS-AND-DOMAINS.md).
