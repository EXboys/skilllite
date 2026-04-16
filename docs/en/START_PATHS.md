# Pick your path

SkillLite is **one repository** with **multiple entry points**. Choose **one** path first; you can adopt the others later.

<a id="path-1-desktop"></a>

### Path 1: Local desktop assistant (SkillLite Assistant)

**You want:** a GUI on your machine — chat, skills, optional IDE layout, governed evolution, local-first workflows.

**Do this:**

- Run or package from source: [crates/skilllite-assistant/README.md](../../crates/skilllite-assistant/README.md) (all `npm` / `tauri` commands run **in that crate directory**).
- **Installers** (dmg / msi / AppImage) ship from [GitHub Releases](https://github.com/EXboys/skilllite/releases) when the [release-desktop workflow](https://github.com/EXboys/skilllite/actions/workflows/release-desktop.yml) has finished for that tag.

The main repo README also has a collapsible **Desktop Assistant** section with feature notes: [README → Desktop](../../README.md) (search for “Desktop Assistant” on the page).

<a id="path-2-sandbox-mcp"></a>

### Path 2: Sandbox and MCP for an existing agent

**You want:** OS-level isolated skill execution inside **Cursor**, **Claude Desktop**, **OpenCode**, or your own stack — without buying into the full SkillLite agent loop first.

**Do this:**

- After `pip install skilllite`, use **`skilllite mcp`** (see [Getting Started](./GETTING_STARTED.md) → CLI commands; MCP may require `pip install skilllite[mcp]`).
- IDE helpers: **`skilllite init-cursor`**, **`skilllite init-opencode`** (see the main README sections for Cursor / OpenCode).
- **Capability map** (CLI, MCP, RPC, binaries): [ENTRYPOINTS-AND-DOMAINS.md](./ENTRYPOINTS-AND-DOMAINS.md).
- **Sandbox-only binary:** build `skilllite-sandbox` from the main README → *Build from Source* / *Build & Install Commands*.

<a id="path-3-fullstack"></a>

### Path 3: Full stack in the terminal or Python

**You want:** the **`skilllite`** CLI, **`from skilllite import chat`**, evolution commands, optional Swarm — the default “engine + agent + sandbox” story.

**Do this:**

- Follow [Getting Started](./GETTING_STARTED.md) from installation step 1.
- For crate boundaries and where features live: [Architecture](./ARCHITECTURE.md).

---

**简体中文：** same structure in [docs/zh/START_PATHS.md](../zh/START_PATHS.md).
