# Pick your path

SkillLite is **one repository** with **multiple entry points**. Pick **one** path to start; you can add the others anytime.

**On this page:** [Path 1 — Desktop](#path-1-desktop) · [Path 2 — Sandbox & MCP](#path-2-sandbox-mcp) · [Path 3 — Full stack](#path-3-fullstack)

[中文版（相同锚点）](../zh/START_PATHS.md)

---

<a id="path-1-desktop"></a>

## Path 1: Local desktop assistant (SkillLite Assistant)

**Goal.** A **GUI** on your machine: chat, skills, optional IDE layout, governed evolution, local-first workflows.

**Start here.**

- From source: [crates/skilllite-assistant/README.md](../../crates/skilllite-assistant/README.md) — all `npm` / `tauri` commands run **only** in that crate directory.
- Installers (**dmg** / **msi** / **AppImage**): [GitHub Releases](https://github.com/EXboys/skilllite/releases), once the [release-desktop](https://github.com/EXboys/skilllite/actions/workflows/release-desktop.yml) workflow has finished for the tag.

**See also.** The main [README](../../README.md) — collapsible **Desktop Assistant** section (search the page for that phrase).

---

<a id="path-2-sandbox-mcp"></a>

## Path 2: Sandbox and MCP for an existing agent

**Goal.** **OS-level** isolated skill execution in **Cursor**, **Claude Desktop**, **OpenCode**, or your own stack — without adopting the full SkillLite agent loop first.

**Start here.**

- After `pip install skilllite`: run **`skilllite mcp`** ([Getting Started](./GETTING_STARTED.md) → CLI; you may need `pip install skilllite[mcp]`).
- Wire the IDE: **`skilllite init-cursor`** · **`skilllite init-opencode`** (Cursor / OpenCode sections in the main [README](../../README.md)).
- Map capabilities (CLI, MCP, RPC, binaries): [ENTRYPOINTS-AND-DOMAINS.md](./ENTRYPOINTS-AND-DOMAINS.md).
- **Sandbox-only** binary: build **`skilllite-sandbox`** from the main README → *Build from Source* / *Build & Install Commands*.

---

<a id="path-3-fullstack"></a>

## Path 3: Full stack in the terminal or Python

**Goal.** The **`skilllite`** CLI, **`from skilllite import chat`**, evolution commands, optional **Swarm** — the default “engine + agent + sandbox” story.

**Start here.**

- Follow [Getting Started](./GETTING_STARTED.md) from installation step 1.
- Crate layout and feature ownership: [Architecture](./ARCHITECTURE.md).
