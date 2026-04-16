# 选择你的路径

SkillLite 是 **同一个仓库**、**多种入口**。建议先只选 **一条** 路径上手，其余以后再加。

<a id="path-1-desktop"></a>

### 路径 1：本地桌面助手（SkillLite Assistant）

**适合你：**想要本机 **图形界面** —— 对话、技能、可选 IDE 三栏、受治理的自进化、本地优先。

**怎么做：**

- 从源码运行或打包：[crates/skilllite-assistant/README.md](../../crates/skilllite-assistant/README.md)（所有 `npm` / `tauri` 命令都在 **该 crate 目录** 下执行）。
- **安装包**（dmg / msi / AppImage）在 [GitHub Releases](https://github.com/EXboys/skilllite/releases) 上，以该 tag 下 [release-desktop 工作流](https://github.com/EXboys/skilllite/actions/workflows/release-desktop.yml) 产物为准。

英文主 README 里折叠区块 **Desktop Assistant** 也有行为说明：[README（英文）](../../README.md)（页内搜索 “Desktop Assistant”）。

<a id="path-2-sandbox-mcp"></a>

### 路径 2：沙箱与 MCP（对接已有 Agent）

**适合你：**已在用 **Cursor**、**Claude Desktop**、**OpenCode** 或自研 Agent，只想接 **OS 级隔离的技能执行**，暂不接完整 SkillLite Agent 循环。

**怎么做：**

- `pip install skilllite` 后使用 **`skilllite mcp`**（见 [快速入门](./GETTING_STARTED.md) 里的 CLI；MCP 可能需要 `pip install skilllite[mcp]`）。
- 一键写配置：**`skilllite init-cursor`**、**`skilllite init-opencode`**（详见英文主 README 的 Cursor / OpenCode 小节）。
- **能力与入口一览**：[ENTRYPOINTS-AND-DOMAINS.md](./ENTRYPOINTS-AND-DOMAINS.md)。
- **仅沙箱二进制：**在英文主 README 的 *Build from Source* / *Build & Install Commands* 中构建 `skilllite-sandbox`。

<a id="path-3-fullstack"></a>

### 路径 3：终端或 Python 全栈

**适合你：**要 **`skilllite` CLI**、**`from skilllite import chat`**、进化相关子命令、可选 Swarm —— 默认的「引擎 + Agent + 沙箱」一体故事。

**怎么做：**

- 从 [快速入门](./GETTING_STARTED.md) 的安装第 1 步跟着做。
- 模块边界与功能落点：[架构说明](./ARCHITECTURE.md)。

---

**English:** [docs/en/START_PATHS.md](../en/START_PATHS.md) (same anchor IDs for cross-linking).
