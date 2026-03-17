# 入口与能力域一览

> 一页建立心智模型：SkillLite 有哪些入口、各依赖哪些 crate、适用场景一句话。便于新人 onboarding 与多入口认知负担控制。

---

## 总览

| 入口 | 是什么 | 依赖的 Crate / 组件 | 适用场景（一句话） |
|------|--------|----------------------|--------------------|
| **CLI** | 主二进制 `skilllite` | core, sandbox, commands, (可选) executor, agent, swarm | 终端用户、脚本、CI：执行技能、扫描、聊天、初始化等全功能。 |
| **Python** | python-sdk + IPC/子进程 | 调用本机 `skilllite` 二进制（`serve` / 子命令） | Python 应用、LangChain/LlamaIndex 集成：scan_code、execute_code、chat、run_skill。 |
| **MCP** | 子命令 `skilllite mcp` | 同 CLI 主二进制（mcp 模块在 skilllite 包内） | Cursor/VSCode 等 IDE：通过 MCP 协议暴露 list_skills、run_skill、scan_code、execute_code。 |
| **Desktop** | skilllite-assistant (Tauri) | skilllite-core（路径/配置）；运行时需已安装 `skilllite` | 桌面用户：图形化聊天、会话管理、读 transcript/memory，背后调 `skilllite`。 |
| **Swarm** | 子命令 `skilllite swarm` | skilllite-swarm（+ 主 binary，agent 时含 swarm_executor） | 多机/多 Agent 组网：mDNS 发现、P2P 任务路由、NewSkill 同步。 |

---

## 1. CLI（主二进制 `skilllite`）

- **入口**：`skilllite`（默认）或轻量 `skilllite-sandbox`（仅沙箱+MCP，无 executor/agent）。
- **依赖**：
  - **必选**：`skilllite-core`、`skilllite-sandbox`、`skilllite-commands`
  - **可选**：`skilllite-executor`（会话/记忆）、`skilllite-agent`（chat/run agent）、`skilllite-swarm`（swarm 子命令）
- **主要子命令**：`run`、`exec`、`scan`、`validate`、`info`、`security-scan`、`bash`、`serve`（IPC）、`chat`、`init`、`mcp`、`swarm`、`evolution`、`init-cursor`、`dependency-audit` 等。
- **适用**：终端与脚本的主力入口；CI、自动化、本地开发。

---

## 2. Python（python-sdk + IPC）

- **入口**：Python 包 `skilllite`（`python-sdk/`），通过 **IPC**（`skilllite serve` stdio JSON-RPC）或 **子进程** 调用本机 `skilllite`。
- **依赖**：无 Rust 直接依赖；运行时依赖已安装的 `skilllite` 二进制（pip 安装或 PATH）。
- **主要 API**：`scan_code`、`execute_code`、`chat`、`run_skill`、`get_binary`；IPC 由 `ipc.py` 连接 `serve`，否则走子进程。
- **适用**：Python 应用、LangChain/LlamaIndex 等框架集成、服务端或本地脚本。

---

## 3. MCP（`skilllite mcp`）

- **入口**：CLI 子命令 `skilllite mcp`，stdio 上跑 MCP（Model Context Protocol）服务器。
- **依赖**：与主二进制相同（skilllite 包内 `mcp/` 模块 + skilllite-commands 等）；不单独成 binary。
- **暴露能力**：list_skills、get_skill_info、run_skill、scan_code、execute_code 等 MCP 工具。
- **适用**：Cursor、VSCode 等 IDE 的 MCP 客户端；配置为启动 `skilllite mcp` 即可。

---

## 4. Desktop（skilllite-assistant）

- **入口**：Tauri 应用 `skilllite-assistant`（需单独构建，不在主 workspace 的 default 构建里）。
- **依赖**：
  - **编译时**：`skilllite-core`（路径、配置等）
  - **运行时**：系统已安装的 `skilllite` 二进制（用于 chat、clear-session、读 transcript 等）
- **能力**：图形化聊天、会话清除、读聊天记录/记忆/输出文件；通过 `skilllite_bridge` 调起 `skilllite`。
- **适用**：不想写命令行的桌面用户、需要常驻托盘与快捷方式的场景。

---

## 5. Swarm（`skilllite swarm`）

- **入口**：CLI 子命令 `skilllite swarm --listen <ADDR>`，长时间运行的 P2P 守护进程。
- **依赖**：`skilllite-swarm`（mDNS、组网、任务路由）；若与 agent 同开，主 binary 提供 `swarm_executor` 在本地执行 NodeTask。
- **能力**：节点发现、任务路由、NewSkill Gossip；可选与 `skilllite run --soul` 等 agent 能力配合。
- **适用**：多机协作、多 Agent 组网、分布式技能发现与执行。

---

## 依赖方向（简要）

- **Core** 不依赖上层；**sandbox / fs / executor / agent / evolution / commands** 依赖 core 或彼此按层级依赖。
- **主二进制** skilllite 聚合 commands + 各可选 feature；**skilllite-assistant** 只依赖 core，运行时依赖外部 `skilllite`；**skilllite-swarm** 仅依赖 core，由主 binary 通过 feature 引入并派发 `swarm` 子命令。

更细的 crate 列表与目录结构见 [ARCHITECTURE.md](./ARCHITECTURE.md)。英文版：[Entry Points and Capability Domains](../en/ENTRYPOINTS-AND-DOMAINS.md)。
