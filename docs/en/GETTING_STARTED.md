# Getting Started

## Installation

### 1. Install Python SDK

```bash
pip install skilllite
```

### 2. Initialize Project

```bash
# Install sandbox binary, create .skills/, and download skills from EXboys/skilllite
skilllite init

# Verify installation
skilllite status
```

Alternatively, manual installation:
```bash
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
from skilllite import chat

# Single-shot agent chat (uses .env for API config)
result = chat("Calculate 15 * 23", skills_dir=".skills")
print(result)
```

For LangChain/LlamaIndex integration, use `langchain-skilllite`:
```bash
pip install langchain-skilllite
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
skilllite init             # Initialize project (sandbox + .skills/ + download skills)
skilllite init --skip-deps # Skip dependency installation
skilllite status           # Check installation status
skilllite add owner/repo   # Add skills from GitHub
skilllite list             # List installed skills
skilllite chat             # Interactive agent chat
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
echo 'export PATH="$HOME/.skilllite/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Manual download

If auto-install fails, download from: https://github.com/EXboys/skilllite/releases

### Building from source

```bash
git clone https://github.com/EXboys/skilllite.git
cd skilllite/skilllite
cargo build --release
cargo install --path .
```

## Next Steps

- Read the [Architecture Guide](./ARCHITECTURE.md) for detailed design
- Check [Contributing Guide](./CONTRIBUTING.md) for contribution
- Explore [benchmark/](../../benchmark/) for performance tests

