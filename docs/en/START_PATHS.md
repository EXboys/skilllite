# Pick your path

SkillLite is **one repository** with **multiple entry points**. Pick **one** path to start; you can add the others anytime.

**Most users should start here:** [Path 2 — Sandbox & MCP](#path-2-sandbox-mcp) (`pip install skilllite` → `skilllite mcp`). You get OS-level skill isolation inside Cursor, Claude Desktop, OpenCode, or your own stack **without** installing the desktop app or adopting the full agent loop.

**On this page:** [Path 2 — Sandbox & MCP](#path-2-sandbox-mcp) · [Path 3 — Full stack](#path-3-fullstack) · [Path 1 — Desktop (optional)](#path-1-desktop)

[中文版（相同锚点）](../zh/START_PATHS.md)

---

<a id="path-2-sandbox-mcp"></a>

## Path 2: Sandbox and MCP for an existing agent (recommended)

**Goal.** **OS-level** isolated skill execution in **Cursor**, **Claude Desktop**, **OpenCode**, or your own stack — without adopting the full SkillLite agent loop first.

**Start here.**

- `pip install skilllite` (MCP extras if needed: `pip install "skilllite[mcp]"`).
- `skilllite init` then **`skilllite mcp`** ([Getting Started](./GETTING_STARTED.md) → CLI).
- Wire the IDE: **`skilllite init-cursor`** · **`skilllite init-opencode`** (Cursor / OpenCode sections in the main [README](../../README.md)).
- Map capabilities (CLI, MCP, RPC, binaries): [ENTRYPOINTS-AND-DOMAINS.md](./ENTRYPOINTS-AND-DOMAINS.md).
- **Sandbox-only** binary: build **`skilllite-sandbox`** from the main README → *Build from Source* / *Build & Install Commands*.
- Tutorial: [06. MCP Server](../../tutorials/06_mcp_server).

---

<a id="path-3-fullstack"></a>

## Path 3: Full stack in the terminal or Python

**Goal.** The **`skilllite`** CLI, **`from skilllite import chat`**, evolution commands, optional **Swarm** — the “engine + agent + sandbox” story when you want SkillLite as the primary runtime.

**Start here.**

- Follow [Getting Started](./GETTING_STARTED.md) from installation step 1.
- Crate layout and feature ownership: [Architecture](./ARCHITECTURE.md).

---

<a id="path-1-desktop"></a>

## Path 1: Local desktop assistant (optional)

**Goal.** A **GUI** on your machine: chat, skills, optional IDE layout, governed evolution, local-first workflows. **Optional** — use Path 2 if you only need sandbox/MCP inside another IDE or agent.

**Layout.** Desktop is a **separate GitHub project**: [EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant). This repo is the engine. Local `skilllite-assistant/` and `crates/skilllite-assistant/` are stubs only.

**Start here.**

- From source: clone [EXboys/skilllite-assistant](https://github.com/EXboys/skilllite-assistant) — all `npm` / `tauri` commands run **only** in that repository.
- Installers (**dmg** / **msi** / **AppImage**): [assistant Releases](https://github.com/EXboys/skilllite-assistant/releases).

**See also.** The main [README](../../README.md) — collapsible **Desktop Assistant** section (search the page for that phrase).
