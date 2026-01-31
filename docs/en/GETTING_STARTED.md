# Getting Started

## Installation

### 1. Install Python SDK

```bash
pip install skilllite
```

### 2. Install Sandbox Binary

SkillLite uses a Rust-based sandbox for secure code execution.

```bash
# Auto-install via CLI
skilllite install

# Or manual installation
curl -fsSL https://raw.githubusercontent.com/EXboys/skilllite/main/install.sh | bash
```

**Supported Platforms:**
- macOS (Intel and Apple Silicon)
- Linux (x86_64 and ARM64)

### 3. Verify Installation

```bash
skilllite status
```

## Quick Usage

### Basic Example

```python
from skilllite import SkillManager
from openai import OpenAI

# Initialize
client = OpenAI(base_url="https://api.deepseek.com/v1", api_key="your_key")
manager = SkillManager(skills_dir="./.skills")

# Get tools
tools = manager.get_tools()

# Call LLM
response = client.chat.completions.create(
    model="deepseek-chat",
    tools=tools,
    messages=[{"role": "user", "content": "Calculate 15 * 23"}]
)

# Handle tool calls
if response.choices[0].message.tool_calls:
    results = manager.handle_tool_calls(response)
```

### Supported LLM Providers

| Provider | base_url |
|----------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot | `https://api.moonshot.cn/v1` |
| Ollama | `http://localhost:11434/v1` |

## CLI Commands

```bash
skilllite install          # Install sandbox binary
skilllite install --force  # Force reinstall
skilllite status           # Check installation status
skilllite version          # Show version info
skilllite uninstall        # Remove installed binary
skilllite mcp              # Start MCP server (requires pip install skilllite[mcp])
```

## Creating Skills

```
my-skill/
├── SKILL.md           # Required: Metadata and docs
├── scripts/
│   └── main.py        # Entry script
├── references/        # Optional: Reference docs
└── assets/            # Optional: Resource files
```

### SKILL.md Example

```markdown
---
name: my-skill
description: My custom skill
compatibility: Requires Python 3.x with requests library, network access
license: MIT
---

# My Skill

This skill does something useful.
```

## Troubleshooting

### Binary not found

```bash
echo 'export PATH="$HOME/.skillbox/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Manual download

If auto-install fails, download from: https://github.com/EXboys/skilllite/releases

### Building from source

```bash
git clone https://github.com/EXboys/skilllite.git
cd skilllite/skillbox
cargo build --release
cargo install --path .
```

## Next Steps

- Read the [Architecture Guide](./ARCHITECTURE.md) for detailed design
- Check [Contributing Guide](./CONTRIBUTING.md) for contribution
- Explore [benchmark/](../../benchmark/) for performance tests

