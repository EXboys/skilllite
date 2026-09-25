# 入口与能力域一览

> 一页建立心智模型：SkillLite 有哪些入口、各依赖哪些 crate、适用场景一句话。便于新人 onboarding 与多入口认知负担控制。

---

## 总览

| 入口 | 是什么 | 依赖的 Crate / 组件 | 适用场景（一句话） |
|------|--------|----------------------|--------------------|
| **CLI** | 主二进制 `skilllite` | core, sandbox, commands, (可选) executor, agent, swarm, artifact HTTP, 统一 gateway 宿主 | 终端用户、脚本、CI：执行技能、扫描、聊天、初始化等全功能。 |
| **Python** | python-sdk + IPC/子进程（artifact 走标准库 HTTP） | 调用本机 `skilllite` 二进制；`artifact_put`/`artifact_get` 对接 artifact HTTP | Python 应用：scan_code、execute_code、chat、run_skill；可选跨进程大对象走 artifact API。 |
| **MCP** | 子命令 `skilllite mcp` | 同 CLI 主二进制（mcp 模块在 skilllite 包内） | Cursor/VSCode 等 IDE：通过 MCP 协议暴露 list_skills、run_skill、scan_code、execute_code。 |
| **Desktop** | skilllite-assistant（Tauri，可选 GUI 项目） | 仅已发布的 `skilllite` 二进制（`agent-rpc` + CLI `--json`）；无引擎 crate path 依赖 | 桌面用户：图形化聊天（含可选 **图片附件** → `agent_chat` 多模态）、会话管理、自进化 UI、运行时供给、transcript/memory 视图。 |
| **Swarm** | 子命令 `skilllite swarm` | skilllite-swarm（+ 主 binary，agent 时含 swarm_executor） | 多机/多 Agent 组网：mDNS 发现、P2P 任务路由、NewSkill 同步。 |

---

## 1. CLI（主二进制 `skilllite`）

- **入口**：`skilllite`（默认）或轻量 `skilllite-sandbox`（仅沙箱+MCP，无 executor/agent）。
- **依赖**：
  - **必选**：`skilllite-core`、`skilllite-sandbox`、`skilllite-commands`
  - **可选**：`skilllite-executor`（会话/记忆）、`skilllite-agent`（chat/run agent）、`skilllite-swarm`（swarm 子命令）
- **主要子命令**：`run`、`exec`、`scan`、`validate`、`info`、`security-scan`、`bash`、`serve`（IPC）、`gateway serve`（统一 HTTP 宿主；**监听**需 `SKILLLITE_GATEWAY_SERVE_ALLOW=1`）、`artifact-serve`（独立 artifact HTTP；**监听**需 `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`）、`channel serve`（独立入站 webhook；**监听**需 `SKILLLITE_CHANNEL_SERVE_ALLOW=1`）、`chat`、`init`、`mcp`、`swarm`、`evolution`、`init-cursor`、`dependency-audit` 等。
- **适用**：终端与脚本的主力入口；CI、自动化、本地开发。

---

## 2. Python（python-sdk + IPC）

- **入口**：Python 包 `skilllite`（`python-sdk/`），通过 **IPC**（`skilllite serve` stdio JSON-RPC）或 **子进程** 调用本机 `skilllite`。
- **依赖**：无 Rust 直接依赖；运行时依赖已安装的 `skilllite` 二进制（pip 安装或 PATH）。
- **主要 API**：`scan_code`、`execute_code`、`chat`、`run_skill`、`get_binary`；`artifact_put` / `artifact_get`（标准库 `urllib`）访问 artifact HTTP（`skilllite gateway serve --artifact-dir ...`、`skilllite artifact-serve` 或任意兼容实现）；IPC 由 `ipc.py` 连接 `serve`，否则走子进程。
- **适用**：Python 应用、LangChain/LlamaIndex 等框架集成、服务端或本地脚本。

---

## 3. MCP（`skilllite mcp`）

- **入口**：CLI 子命令 `skilllite mcp`，stdio 上跑 MCP（Model Context Protocol）服务器。
- **依赖**：与主二进制相同（skilllite 包内 `mcp/` 模块 + skilllite-commands 等）；不单独成 binary。
- **暴露能力**：list_skills、get_skill_info、run_skill、scan_code、execute_code 等 MCP 工具。
- **适用**：Cursor、VSCode 等 IDE 的 MCP 客户端；配置为启动 `skilllite mcp` 即可。

---

## 4. Desktop（skilllite-assistant）

- **产品角色**：引擎的**可选 GUI 分发**，不是默认对接路径（默认见 [路径 2 — 沙箱与 MCP](./START_PATHS.md#path-2-sandbox-mcp)）。已迁入**独立仓库**（[Assistant 可拆仓架构](./ASSISTANT-SPLIT-ARCHITECTURE.md)）。
- **入口**：[EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant)。在该仓执行 `npm run tauri build`。引擎仓 `skilllite-assistant/` 与 `crates/skilllite-assistant/` 仅为 stub。
- **集成模型** — 仅三层：
  - **L1** `skilllite agent-rpc` — 流式聊天、确认/澄清。
  - **L2** `skilllite … --json` — 进化面板、运行时安装、技能列表。
  - **L3** 工作区文件 — prompts/transcript 等允许路径的读写。
- **依赖（D1′）**：**不再** path 依赖引擎 crate；仅 semver 钉扎的 **`skilllite` 二进制**。预构建通过 `SKILLLITE_ENGINE_ROOT` / 向上查找引擎仓，或复制已安装二进制。
- **能力**：图形聊天、会话、进化审核/触发、运行时安装、IDE 三栏、多模态 `agent_chat`。
- **适用**：不想在 IDE 里配 MCP、需要本机 App/托盘的用户。
- **布局**：P4 已发布为 [EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant)。引擎仓保留 stub 与文档指针。

---

## 5. Swarm（`skilllite swarm`）

- **入口**：CLI 子命令 `skilllite swarm --listen <ADDR>`，长时间运行的 P2P 守护进程。默认监听 `127.0.0.1:7700`（回环）。局域网访问使用 `0.0.0.0:<端口>`；请设置 `SKILLLITE_SWARM_TOKEN`，要求 HTTP 客户端携带 `Authorization: Bearer`。
- **依赖**：`skilllite-swarm`（mDNS、组网、任务路由）；若与 agent 同开，主 binary 提供 `swarm_executor` 在本地执行 NodeTask。
- **能力**：节点发现、任务路由、NewSkill Gossip；可选与 `skilllite run --soul` 等 agent 能力配合。
- **适用**：多机协作、多 Agent 组网、分布式技能发现与执行。

---

## 依赖方向（简要）

- **Core** 不依赖上层；**sandbox / fs / executor / agent / evolution / commands** 依赖 core 或彼此按层级依赖。
- **主二进制** skilllite 聚合 commands + 各可选 feature；**skilllite-assistant** 是可选 GUI，只通过 `skilllite` 二进制通信；**skilllite-swarm** 仅依赖 core，由主 binary 通过 feature 引入并派发 `swarm` 子命令。

更细的 crate 列表与目录结构见 [ARCHITECTURE.md](./ARCHITECTURE.md)。英文版：[Entry Points and Capability Domains](../en/ENTRYPOINTS-AND-DOMAINS.md)。
