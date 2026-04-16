# 选择你的路径

SkillLite 是 **同一个仓库**、**多种入口**。建议先选 **一条** 路径上手，其余随时再补。

**本页导航：** [路径 1 — 桌面](#path-1-desktop) · [路径 2 — 沙箱与 MCP](#path-2-sandbox-mcp) · [路径 3 — 全栈](#path-3-fullstack)

[English (same anchors)](../en/START_PATHS.md)

---

<a id="path-1-desktop"></a>

## 路径 1：本地桌面助手（SkillLite Assistant）

**目标。** 在本机用 **图形界面**：对话、技能、可选 IDE 三栏、受治理的自进化、本地优先工作流。

**从这里开始。**

- 源码运行与打包：[crates/skilllite-assistant/README.md](../../crates/skilllite-assistant/README.md) — 所有 `npm` / `tauri` 命令 **只** 在该 crate 目录下执行。
- 安装包（**dmg** / **msi** / **AppImage**）：见 [GitHub Releases](https://github.com/EXboys/skilllite/releases)，需对应 tag 的 [release-desktop](https://github.com/EXboys/skilllite/actions/workflows/release-desktop.yml) 工作流已跑完。

**另见。** 英文主 [README](../../README.md) 中的折叠区块 **Desktop Assistant**（页内搜索该英文标题）。

---

<a id="path-2-sandbox-mcp"></a>

## 路径 2：沙箱与 MCP（对接已有 Agent）

**目标。** 在 **Cursor**、**Claude Desktop**、**OpenCode** 或自研 Agent 里接入 **操作系统级** 隔离的技能执行 —— 暂不必接入完整 SkillLite Agent 循环。

**从这里开始。**

- `pip install skilllite` 后运行 **`skilllite mcp`**（见 [快速入门](./GETTING_STARTED.md) 中的 CLI；可能需要 `pip install skilllite[mcp]`）。
- 接入 IDE：**`skilllite init-cursor`** · **`skilllite init-opencode`**（见英文主 [README](../../README.md) 的 Cursor / OpenCode 小节）。
- 能力与入口一览（CLI、MCP、RPC、各二进制）：[ENTRYPOINTS-AND-DOMAINS.md](./ENTRYPOINTS-AND-DOMAINS.md)。
- **仅沙箱** 二进制：在英文主 README 的 *Build from Source* / *Build & Install Commands* 中构建 **`skilllite-sandbox`**。

---

<a id="path-3-fullstack"></a>

## 路径 3：终端或 Python 全栈

**目标。** **`skilllite` CLI**、**`from skilllite import chat`**、进化相关子命令、可选 **Swarm** —— 默认的「引擎 + Agent + 沙箱」一体路径。

**从这里开始。**

- 从 [快速入门](./GETTING_STARTED.md) 的安装第 1 步跟着做。
- Crate 划分与功能归属：[架构说明](./ARCHITECTURE.md)。
