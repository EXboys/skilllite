# AGENTS.md

## Cursor Cloud specific instructions

### Overview

SkillLite is a Rust workspace (`Cargo.toml` at repo root) with a Python SDK (`python-sdk/`). The main binary is `skilllite` built from `skilllite/` crate. The desktop app (`crates/skilllite-assistant/`) is excluded from the workspace and is optional.

### Running checks (mirrors CI)

Rust:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check bans
```

Python SDK (from `python-sdk/`):
```bash
ruff check .
ruff format --check .
mypy skilllite
pytest
```

### Building

```bash
cargo build -p skilllite          # full binary (debug)
cargo build -p skilllite --release  # release binary
```

### Running skills

```bash
# Sandbox Level 3 (default) requires kernel namespace support (bwrap).
# In Cloud Agent VMs (nested containers), bwrap namespace creation fails with
# "Resource temporarily unavailable". Use Level 1 for testing:
SKILLLITE_SANDBOX_LEVEL=1 ./target/debug/skilllite run .skills/calculator '{"operation":"add","a":1,"b":2}'

# For skills that trigger security scan (non-TTY stdin blocks approval):
SKILLLITE_SANDBOX_LEVEL=1 SKILLLITE_AUTO_APPROVE=1 ./target/debug/skilllite run .skills/text-processor '{"text":"test","operation":"uppercase"}'
```

### Key gotchas

- **Sandbox level in Cloud VMs**: `bwrap` (bubblewrap) requires unprivileged user namespaces which are unavailable inside the Cloud Agent container. Set `SKILLLITE_SANDBOX_LEVEL=1` for functional testing of skill execution logic. Security sandbox integration tests should be validated in CI (ubuntu-latest has namespace support).
- **python3-venv**: Required for skill execution (`ensure_environment` creates venvs). Install with `sudo apt-get install -y python3.12-venv`.
- **PATH for Python dev tools**: `ruff`, `mypy`, `pytest` install to `/home/ubuntu/.local/bin` — ensure it's on PATH.
- **Rust toolchain**: The project uses latest stable Rust. Run `rustup default stable` if needed.
- **Desktop app (Tauri)**: Not part of the default workspace (`exclude = ["crates/skilllite-assistant"]`). Requires Node.js + GTK libs on Linux. Skip unless specifically needed.
