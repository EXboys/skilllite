# SkillLite Rust Binary

A lightweight Skills secure execution engine for Linux and macOS.

> This package is part of a **Cargo Workspace**. Build from the **repository root**.

## Build & Install Reference

All commands run from **repository root** (or use `cd skilllite && cargo install --path .`).

### Build commands

| Binary | Command | What you get |
|--------|---------|--------------|
| **skilllite** | `cargo build -p skilllite` | **Full** (Agent + Chat + MCP + sandbox + audit). No vector memory. |
| **skilllite** | `cargo build -p skilllite --features memory_vector` | Full **+ vector memory** search |
| **skilllite** | `cargo build -p skilllite --no-default-features` | Minimal: run/exec/bash/scan only |
| **skilllite-sandbox** | `cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | Sandbox + MCP only |

### Install commands

| Command | Binary installed |
|---------|------------------|
| `cargo install --path skilllite` | **skilllite** — full. Does **not** include vector memory. |
| `cargo install --path skilllite --features memory_vector` | **skilllite** — full + vector memory |
| `cargo install --path skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | **skilllite-sandbox** |

Default features = `sandbox`, `audit`, `agent`. `memory_vector` is optional.

### Output paths

- Install: `~/.cargo/bin/skilllite` or `~/.cargo/bin/skilllite-sandbox`
- Build: `target/release/skilllite` or `target/release/skilllite-sandbox`

## Project Structure (Workspace)

```
skilllite/           # Repository root
├── Cargo.toml       # [workspace] members = ["skilllite", "crates/*"]
├── skilllite/       # This dir: main binary entry
│   └── src/         # main.rs, cli, commands, stdio_rpc, mcp
└── crates/
    ├── skilllite-core/      # Config, skill, path_validation, observability
    ├── skilllite-sandbox/   # Sandbox + env (independently addable via cargo add)
    ├── skilllite-executor/  # Session, transcript, memory
    └── skilllite-agent/     # LLM, tools, chat, extensions
```

`crates/` follows the common Rust multi-crate layout (e.g. tokio, clap, serde), enabling:
- Enforced dependency direction and no circular deps
- Independent publishing (e.g. `skilllite-sandbox` via `cargo add`)
- Faster incremental builds

## Features

- **SKILL.md Parsing**: Parse skill metadata from YAML front matter with auto-detection of entry points and language
- **Dependency Management**: Automatic Python venv and Node.js node_modules setup with caching
- **Multi-level Sandbox Execution**:
  - **Level 1**: No sandbox - direct execution
  - **Level 2**: Sandbox isolation only (macOS Seatbelt / Linux namespace + seccomp)
  - **Level 3**: Sandbox isolation + static code scanning (default)
- **Security Scanning**: Static code analysis with customizable rules for Python and JavaScript
- **Network Control**: Configurable network access derived from compatibility field
- **Resource Limits**: Memory limits (default 256MB) and timeout controls (default 30s)
- **JSON Protocol**: stdin/stdout JSON communication

## Installation

```bash
cargo install --path .
```

## CLI Commands

### Skill Management

| Command | Description | Example |
|---------|-------------|---------|
| `skilllite list` | List installed skills | `skilllite list -s .skills --json` |
| `skilllite add` | Add skill from GitHub/local | `skilllite add owner/repo` |
| `skilllite remove` | Remove installed skill | `skilllite remove my-skill` |
| `skilllite show` | Show skill details | `skilllite show my-skill -s .skills` |
| `skilllite init` | Initialize project (sandbox + .skills/) | `skilllite init` |
| `skilllite reindex` | Rescan skills directory | `skilllite reindex -s .skills` |

### Skill Execution

| Command | Description | Example |
|---------|-------------|---------|
| `skilllite run` | Run skill (requires entry_point) | `skilllite run ./.skills/calculator '{"a": 1, "b": 2}'` |
| `skilllite exec` | Execute script directly (no entry_point) | `skilllite exec ./.skills/my-skill scripts/main.py '{}'` |
| `skilllite bash` | Execute bash-tool skill command | `skilllite bash ./.skills/browser-tool 'agent-browser --help'` |

### Tools & Adapters

| Command | Description | Example |
|---------|-------------|---------|
| `skilllite list-tools` | Output OpenAI/Claude tool definitions | `skilllite list-tools -s .skills --format openai` |
| `skilllite mcp` | Run MCP server (Cursor/Claude) | `skilllite mcp` |
| `skilllite init-cursor` | Initialize Cursor IDE integration | `skilllite init-cursor` |
| `skilllite init-opencode` | Initialize OpenCode integration | `skilllite init-opencode` |

### Agent & Chat

| Command | Description |
|---------|-------------|
| `skilllite chat` | Interactive chat with LLM agent |
| `skilllite quickstart` | Auto-detect LLM, setup skills, launch chat |

### Other Commands

| Command | Description |
|---------|-------------|
| `skilllite scan` | Scan skill dir, list executable scripts (JSON) |
| `skilllite validate` | Validate skill structure |
| `skilllite info` | Show skill metadata |
| `skilllite security-scan` | Security scan a script |
| `skilllite dependency-audit` | Audit skill dependencies for vulnerabilities |
| `skilllite clean-env` | Clean cached virtual environments |

## Usage Details

### Run a Skill

```bash
skilllite run <SKILL_DIR> '<INPUT_JSON>' [OPTIONS]
```

**Options:**
- `--allow-network`: Override SKILL.md network policy
- `--cache-dir <DIR>`: Custom cache directory for environments
- `--max-memory <MB>`: Maximum memory limit in MB (default: 256)
- `--timeout <SECS>`: Execution timeout in seconds (default: 30)
- `--sandbox-level <1|2|3>`: Sandbox security level (default: 3)

**Example:**
```bash
skilllite run ./examples/python_skill '{"message": "Hello, World!"}'
```

### Execute a Script Directly

```bash
skilllite exec <SKILL_DIR> <SCRIPT_PATH> '<INPUT_JSON>' [OPTIONS]
```

Execute a specific script without requiring an entry_point in SKILL.md.

**Options:**
- `--args <ARGS>`: Script arguments
- `--allow-network`: Allow network access
- `--sandbox-level <1|2|3>`: Sandbox security level

### Scan a Skill

```bash
skilllite scan <SKILL_DIR> [--preview-lines <N>]
```

List all executable scripts in a skill directory (JSON output for LLM analysis).

### Validate a Skill

```bash
skilllite validate <SKILL_DIR>
```

### Show Skill Info

```bash
skilllite info <SKILL_DIR>
```

### Security Scan a Script

```bash
skilllite security-scan <SCRIPT_PATH> [OPTIONS]
```

**Options:**
- `--allow-network`: Allow network operations
- `--allow-file-ops`: Allow file operations
- `--allow-process-exec`: Allow process execution

## SKILL.md Format

SkillLite follows the [Claude Agent Skills Specification](https://docs.anthropic.com/en/docs/agents-and-tools/agent-skills/specification).

```yaml
---
name: my-skill
description: My awesome skill
compatibility: "Requires Python 3.x, network access"
license: MIT
metadata:
  author: Your Name
  version: "1.0.0"
---

# My Skill

Description of the skill...
```

**Key Fields:**
- `name`: Skill name (max 64 chars, lowercase + hyphens)
- `description`: What the skill does (max 1024 chars)
- `compatibility`: Environment requirements (auto-detects language and network needs)
- `license`: License name or reference
- `metadata`: Additional metadata (author, version, etc.)

**Auto-detection:**
- Entry point is auto-detected from `scripts/main.{py,js,ts,sh}`
- Language is detected from compatibility field or entry point extension
- Network access is enabled if compatibility mentions "network", "internet", "http", "api", or "web"

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SKILLBOX_SANDBOX_LEVEL` | Sandbox security level (1-3) | 3 |
| `SKILLBOX_MAX_MEMORY_MB` | Maximum memory in MB | 256 |
| `SKILLBOX_TIMEOUT_SECS` | Execution timeout in seconds | 30 |
| `SKILLBOX_AUTO_APPROVE` | Auto-approve security prompts (1/true/yes) | - |
| `SKILLBOX_CACHE_DIR` | Custom cache directory | System cache |

> Full env var list: [ENV_REFERENCE](../docs/en/ENV_REFERENCE.md) / [ENV_REFERENCE 中文](../docs/zh/ENV_REFERENCE.md).

## Custom Security Rules

Create a `.skilllite-rules.yaml` file in your skill directory:

```yaml
use_default_rules: true
disabled_rules:
  - py-file-open
rules:
  - id: custom-rule
    pattern: "dangerous_function\\s*\\("
    issue_type: code_injection
    severity: high
    description: "Custom dangerous function"
    languages: ["python"]
```

## Python Integration

```python
import subprocess
import json

def run_skill(skill_dir: str, input_data: dict) -> dict:
    result = subprocess.run(
        ["skilllite", "run", skill_dir, json.dumps(input_data)],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)

# Usage
output = run_skill("./my_skill", {"param": "value"})
```

## License

MIT
