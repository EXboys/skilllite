# Entry Points and Capability Domains

> One-page mental model: what entry points SkillLite has, which crates each depends on, and a one-line use case. Reduces onboarding cost and cognitive load from multiple entry points.

---

## Overview

| Entry | What it is | Crate / component dependencies | Use case (one line) |
|-------|-------------|---------------------------------|----------------------|
| **CLI** | Main binary `skilllite` | core, sandbox, commands, (optional) executor, agent, swarm | Terminal users, scripts, CI: run skills, scan, chat, init, and full feature set. |
| **Python** | python-sdk + IPC/subprocess | Calls local `skilllite` binary (`serve` / subcommands) | Python apps, LangChain/LlamaIndex: scan_code, execute_code, chat, run_skill. |
| **MCP** | Subcommand `skilllite mcp` | Same as CLI main binary (mcp module lives in skilllite package) | Cursor/VSCode etc.: MCP protocol exposes list_skills, run_skill, scan_code, execute_code. |
| **Desktop** | skilllite-assistant (Tauri) | skilllite-core (paths/config); runtime requires installed `skilllite` | Desktop users: GUI chat, session management, read transcript/memory via `skilllite`. |
| **Swarm** | Subcommand `skilllite swarm` | skilllite-swarm (+ main binary; with agent, includes swarm_executor) | Multi-machine / multi-agent: mDNS discovery, P2P task routing, NewSkill sync. |

---

## 1. CLI (main binary `skilllite`)

- **Entry**: `skilllite` (default) or lightweight `skilllite-sandbox` (sandbox + MCP only, no executor/agent).
- **Dependencies**:
  - **Required**: `skilllite-core`, `skilllite-sandbox`, `skilllite-commands`
  - **Optional**: `skilllite-executor` (session/memory), `skilllite-agent` (chat/run agent), `skilllite-swarm` (swarm subcommand)
- **Main subcommands**: `run`, `exec`, `scan`, `validate`, `info`, `security-scan`, `bash`, `serve` (IPC), `chat`, `init`, `mcp`, `swarm`, `evolution`, `init-cursor`, `dependency-audit`, etc.
- **Use case**: Primary terminal and script entry; CI, automation, local development.

---

## 2. Python (python-sdk + IPC)

- **Entry**: Python package `skilllite` (`python-sdk/`), calling local `skilllite` via **IPC** (`skilllite serve` stdio JSON-RPC) or **subprocess**.
- **Dependencies**: No direct Rust dependency; runtime depends on an installed `skilllite` binary (pip or PATH).
- **Main API**: `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`; IPC in `ipc.py` connects to `serve`, otherwise subprocess.
- **Use case**: Python applications, LangChain/LlamaIndex integration, server or local scripts.

---

## 3. MCP (`skilllite mcp`)

- **Entry**: CLI subcommand `skilllite mcp`, running an MCP (Model Context Protocol) server over stdio.
- **Dependencies**: Same as main binary (skilllite package `mcp/` module + skilllite-commands, etc.); not a separate binary.
- **Exposed tools**: list_skills, get_skill_info, run_skill, scan_code, execute_code, etc.
- **Use case**: Cursor, VSCode, and other MCP-capable IDEs; configure to start `skilllite mcp`.

---

## 4. Desktop (skilllite-assistant)

- **Entry**: Tauri app `skilllite-assistant` (built separately; not part of main workspace default build).
- **Dependencies**:
  - **Build time**: `skilllite-core` (paths, config, etc.)
  - **Runtime**: System-installed `skilllite` binary (for chat, clear-session, reading transcript, etc.)
- **Capabilities**: GUI chat, session clear, read chat history / memory / output files; invokes `skilllite` via `skilllite_bridge`.
- **Use case**: Desktop users who prefer not to use the CLI; tray and shortcut scenarios.

---

## 5. Swarm (`skilllite swarm`)

- **Entry**: CLI subcommand `skilllite swarm --listen <ADDR>`, long-running P2P daemon.
- **Dependencies**: `skilllite-swarm` (mDNS, mesh, task routing); when used with agent, main binary provides `swarm_executor` to run NodeTasks locally.
- **Capabilities**: Node discovery, task routing, NewSkill Gossip; optionally used with agent features like `skilllite run --soul`.
- **Use case**: Multi-machine collaboration, multi-agent mesh, distributed skill discovery and execution.

---

## Dependency direction (brief)

- **Core** does not depend on upper layers; **sandbox / fs / executor / agent / evolution / commands** depend on core or each other by layer.
- **Main binary** skilllite aggregates commands and optional features; **skilllite-assistant** depends only on core at build time and on external `skilllite` at runtime; **skilllite-swarm** depends only on core and is wired in by the main binary via feature for the `swarm` subcommand.

For detailed crate list and directory layout, see [ARCHITECTURE.md](./ARCHITECTURE.md). 中文版：[入口与能力域一览](../zh/ENTRYPOINTS-AND-DOMAINS.md).
