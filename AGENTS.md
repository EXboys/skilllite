# AGENTS.md

## Cursor Cloud specific instructions

### Project overview

SkillLite is a lightweight AI Agent Skills secure execution engine with a Rust core and Python SDK bridge. See `README.md` for full documentation.

### Services

| Service | Description | How to run |
|---------|-------------|------------|
| **Rust CLI** (`skilllite`) | Core binary — sandbox executor, CLI, agent loop, MCP server | `cargo build -p skilllite` (binary at `target/debug/skilllite`) |
| **Python SDK** | Thin bridge (~600 lines) calling the Rust binary via subprocess | `cd python-sdk && pip install -e .` |
| **Desktop Assistant** (optional) | Tauri 2 + React desktop app | Requires GTK/GDK libs; see `crates/skilllite-assistant/README.md` |

### Lint / Test / Build commands

See `docs/en/CONTRIBUTING.md` for the canonical reference. Key commands:

- **Rust format**: `cargo fmt --all -- --check`
- **Rust clippy**: `cargo clippy -p skilllite -p skilllite-core -p skilllite-sandbox -p skilllite-executor -p skilllite-agent`
- **Rust tests**: `cargo test -p skilllite -p skilllite-core -p skilllite-sandbox -p skilllite-executor -p skilllite-agent`
- **Rust build**: `cargo build -p skilllite`

Use `-p` flags to exclude the Tauri desktop crate (`skilllite-assistant`) which needs GTK/GDK system libs not available in the Cloud VM.

### Non-obvious gotchas

1. **Rust toolchain version**: Dependencies require Rust >= 1.88. The update script ensures the latest stable is installed. The environment has both `/usr/local/cargo/bin/` and `/root/.cargo/bin/` — set `RUSTUP_HOME=/root/.rustup CARGO_HOME=/root/.cargo` and prepend `/root/.cargo/bin` to `PATH`.

2. **Python SDK namespace collision**: When running Python from `/workspace/`, the `skilllite/` Rust crate directory shadows the installed `skilllite` Python package. Either run Python from a different directory, or use `sys.path.insert(0, '/workspace/python-sdk')` before importing.

3. **Sandbox not available in Cloud VM**: The kernel lacks `CONFIG_SECCOMP` support. To run skills, set `SKILLBOX_NO_SANDBOX=1` and use `--sandbox-level 1`. Also set `SKILLLITE_TRUST_BYPASS_CONFIRM=1` to bypass trust confirmation prompts.

4. **python3-venv required**: The skill executor creates virtual environments for Python skills. Ensure `python3-venv` is installed (`sudo apt-get install -y python3-venv`).

5. **Clippy scope**: Run clippy only on the 5 core crates (not workspace-wide) to avoid GTK dependency errors from the Tauri crate.
