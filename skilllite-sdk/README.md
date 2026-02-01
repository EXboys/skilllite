# SkillLite Python SDK

A Python SDK for the SkillLite execution engine, using **OpenAI-compatible API format** as the unified interface.

## Supported Providers

Works with any OpenAI-compatible LLM provider:
- **OpenAI** (GPT-4, GPT-3.5, etc.)
- **Azure OpenAI**
- **Anthropic Claude** (via OpenAI-compatible endpoint or native)
- **Google Gemini** (via OpenAI-compatible endpoint)
- **Local models** (Ollama, vLLM, LMStudio, etc.)
- **DeepSeek, Qwen, Moonshot, Zhipu**, and other providers

## Installation

```bash
pip install skilllite

# With OpenAI SDK (recommended, works with all compatible providers)
pip install skilllite[openai]

# With Anthropic SDK (for Claude's native API)
pip install skilllite[anthropic]
```

## Prerequisites

You need to have the `skillbox` binary installed:

```bash
# From the skillbox directory
cargo install --path .
```

## Quick Start

### Basic Usage (Universal - Works with Any Provider)

```python
from openai import OpenAI
from skilllite import SkillManager

# Works with ANY OpenAI-compatible provider
# Just change base_url and api_key for different providers:

# OpenAI
client = OpenAI()

# Ollama (local)
# client = OpenAI(base_url="http://localhost:11434/v1", api_key="ollama")

# DeepSeek
# client = OpenAI(base_url="https://api.deepseek.com/v1", api_key="...")

# Qwen (通义千问)
# client = OpenAI(base_url="https://dashscope.aliyuncs.com/compatible-mode/v1", api_key="...")

# Moonshot (月之暗面)
# client = OpenAI(base_url="https://api.moonshot.cn/v1", api_key="...")

# Initialize SkillManager
manager = SkillManager(skills_dir="./my_skills")

# Get tools (OpenAI-compatible format - works with all providers)
tools = manager.get_tools()

# Call any OpenAI-compatible API
response = client.chat.completions.create(
    model="gpt-4",  # or "llama2", "deepseek-chat", "qwen-turbo", etc.
    tools=tools,
    messages=[{"role": "user", "content": "Please help me with..."}]
)

# Handle tool calls (same code works for all providers!)
if response.choices[0].message.tool_calls:
    tool_results = manager.handle_tool_calls(response)
    
    # Continue conversation with results
    messages = [
        {"role": "user", "content": "Please help me with..."},
        response.choices[0].message,
        *[r.to_openai_format() for r in tool_results]
    ]
    
    follow_up = client.chat.completions.create(
        model="gpt-4",
        tools=tools,
        messages=messages
    )
```

### Agentic Loop (Automatic Multi-turn Tool Execution)

```python
from openai import OpenAI
from skilllite import SkillManager

# Works with any provider
client = OpenAI()  # or OpenAI(base_url="...", api_key="...")
manager = SkillManager(skills_dir="./my_skills")

# Create an agentic loop
loop = manager.create_agentic_loop(
    client=client,
    model="gpt-4",
    system_prompt="You are a helpful assistant with access to various skills.",
    max_iterations=10,
    temperature=0.7  # Additional kwargs passed to chat.completions.create()
)

# Run until completion - handles multiple tool calls automatically
final_response = loop.run("Please analyze this data and generate a report.")
print(final_response.choices[0].message.content)
```

### Claude Native API (Optional)

If you prefer using Claude's native API directly:

```python
import anthropic
from skilllite import SkillManager

client = anthropic.Anthropic()
manager = SkillManager(skills_dir="./my_skills")

# Use Claude-specific methods
tools = manager.get_tools_for_claude_native()
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    tools=tools,
    messages=[{"role": "user", "content": "..."}]
)

if response.stop_reason == "tool_use":
    results = manager.handle_tool_calls_claude_native(response)
```

## Creating Skills

Skills are defined in directories with a `SKILL.md` file:

```
my_skills/
├── web-search/
│   ├── SKILL.md           # Metadata and docs (includes dependency declaration)
│   └── scripts/
│       └── main.py
└── calculator/
    ├── SKILL.md
    └── scripts/
        └── main.py
```

> **Note**: Python dependencies are declared in the `compatibility` field of `SKILL.md`, not in a separate `requirements.txt` file.

### SKILL.md Format

```markdown
---
name: web-search
description: Search the web for information
compatibility: Requires Python 3.x with requests library, network access
license: MIT
metadata:
  author: example-org
  version: "1.0"
---

# Web Search Skill

This skill searches the web for information.

## Input Parameters

- `query`: The search query (required)
```

The `compatibility` field is used to:
- Detect language (Python/Node/Bash)
- Enable network access (keywords: network, internet, http, api, web)
- Auto-install dependencies (known packages like requests, pandas, axios, etc.)

### Skill Entry Point

```python
# main.py
import json
import sys

def main():
    # Read input from stdin
    input_data = json.loads(sys.stdin.read())
    
    # Process the input
    query = input_data.get("query", "")
    
    # Do something...
    result = {"results": [f"Result for: {query}"]}
    
    # Output JSON to stdout
    print(json.dumps(result))

if __name__ == "__main__":
    main()
```

## API Reference

### SkillManager

The main class for managing and executing skills.

#### Constructor

```python
SkillManager(
    skills_dir: Optional[str] = None,  # Directory containing skills
    binary_path: Optional[str] = None,  # Path to skillbox binary
    cache_dir: Optional[str] = None,    # Cache directory for venvs
    allow_network: bool = False          # Default network access
)
```

#### Methods

- `scan_directory(directory)` - Scan for skills
- `register_skill(skill_dir)` - Register a single skill
- `get_skill(name)` - Get skill by name
- `list_skills()` - List all skills
- `get_tools_for_claude()` - Get Claude-format tools
- `get_tools_for_openai()` - Get OpenAI-format tools
- `execute(skill_name, input_data)` - Execute a skill
- `handle_tool_calls(response, format)` - Handle LLM tool calls
- `create_agentic_loop(...)` - Create an agentic loop

### ToolFormat

Enum for LLM provider formats:
- `ToolFormat.CLAUDE`
- `ToolFormat.OPENAI`

## OpenCode Integration

SkillLite can be integrated with [OpenCode](https://github.com/opencode-ai/opencode) as an MCP (Model Context Protocol) server, providing secure sandbox execution capabilities.

### Quick Setup

```bash
# Install with MCP support
pip install skilllite[mcp]

# One-command setup for OpenCode
skilllite init-opencode

# Start OpenCode
opencode
```

The `init-opencode` command automatically:
- Detects the best way to start the MCP server (uvx, pipx, skilllite, or python)
- Creates `opencode.json` with optimal configuration
- Generates `.opencode/skills/skilllite/SKILL.md` with usage instructions
- Discovers your pre-defined skills

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `skilllite_list_skills` | List all available skills |
| `skilllite_get_skill_info` | Get skill details and input schema |
| `skilllite_run_skill` | Execute a pre-defined skill |
| `skilllite_scan_code` | Scan code for security issues |
| `skilllite_execute_code` | Execute code in secure sandbox |

### Security Features

- **System-level Sandbox**: macOS Seatbelt / Linux Namespace isolation
- **Security Scanning**: Static analysis before execution
- **User Confirmation**: Dangerous code requires explicit approval
- **Scan ID Verification**: Prevents code modification between scan and execution

For detailed documentation, see [OpenCode Integration Tutorial](../tutorials/07_opencode_integration/README.md).

## CLI Commands

```bash
skilllite install        # Install skillbox sandbox binary
skilllite uninstall      # Remove skillbox binary
skilllite status         # Show installation status
skilllite version        # Show version information
skilllite mcp            # Start MCP server
skilllite init-opencode  # Initialize OpenCode integration
```

## License

MIT License
