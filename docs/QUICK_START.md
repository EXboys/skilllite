# Quick Start Guide

## Installation

### 1. Install Python SDK

```bash
pip install skilllite
```

### 2. Install Sandbox Binary

SkillLite uses a Rust-based sandbox for secure code execution. Install it with:

```bash
skilllite install
```

This will download and install the pre-built `skillbox` binary for your platform.

**Supported Platforms:**
- macOS (Intel and Apple Silicon)
- Linux (x86_64 and ARM64)

### 3. Verify Installation

```bash
skilllite status
```

You should see:
```
SkillLite Installation Status
========================================
✓ skillbox is installed (v0.1.0)
  Location: /Users/username/.skillbox/bin/skillbox
```

## Usage

### Basic Example

```python
from skilllite import SkillManager

# Initialize the skill manager
manager = SkillManager(skills_dir=".skills")

# The sandbox binary will be auto-installed if not found
# Execute a skill
result = manager.execute_skill(
    skill_name="calculator",
    params={"expression": "2 + 2"}
)

print(result)  # Output: {"result": 4}
```

### With OpenAI

```python
from skilllite import SkillManager
from openai import OpenAI

# Initialize
manager = SkillManager(skills_dir=".skills")
client = OpenAI()

# Get tools for LLM
tools = manager.get_tools()

# Chat with function calling
response = client.chat.completions.create(
    model="gpt-4",
    messages=[
        {"role": "user", "content": "Calculate 15 * 23"}
    ],
    tools=tools
)

# Execute the tool call
if response.choices[0].message.tool_calls:
    tool_call = response.choices[0].message.tool_calls[0]
    result = manager.execute_tool_call(tool_call)
    print(result)
```

### With Claude (Anthropic)

```python
from skilllite import SkillManager
from anthropic import Anthropic

# Initialize
manager = SkillManager(skills_dir=".skills")
client = Anthropic()

# Get tools for Claude
tools = manager.get_tools(format="anthropic")

# Chat with tool use
response = client.messages.create(
    model="claude-3-5-sonnet-20241022",
    max_tokens=1024,
    tools=tools,
    messages=[
        {"role": "user", "content": "What's the weather in San Francisco?"}
    ]
)

# Execute tool calls
if response.stop_reason == "tool_use":
    for content in response.content:
        if content.type == "tool_use":
            result = manager.execute_skill(
                skill_name=content.name,
                params=content.input
            )
            print(result)
```

## CLI Commands

### Install Sandbox

```bash
# Install latest version
skilllite install

# Install specific version
skilllite install --version 0.1.0

# Force reinstall
skilllite install --force
```

### Check Status

```bash
skilllite status
```

### Show Version

```bash
skilllite version
```

### Uninstall

```bash
skilllite uninstall
```

### Start MCP Server

```bash
# Install MCP support first
pip install skilllite[mcp]

# Start MCP server
skilllite mcp
```

## Creating Skills

Create a new skill directory:

```
.skills/
  my-skill/
    SKILL.md          # Skill metadata and documentation
    scripts/
      main.py         # Main execution script
```

Example `SKILL.md`:

```markdown
---
name: my-skill
description: A custom skill
version: 1.0.0
runtime: python
entry: scripts/main.py
---

# My Skill

This skill does something useful.

## Parameters

- `input`: The input data

## Returns

- `output`: The processed result
```

Example `scripts/main.py`:

```python
import json
import sys

def main():
    # Read input from stdin
    input_data = json.loads(sys.stdin.read())
    
    # Process
    result = {
        "output": f"Processed: {input_data.get('input', '')}"
    }
    
    # Write output to stdout
    print(json.dumps(result))

if __name__ == "__main__":
    main()
```

## Next Steps

- Read the [Installation Guide](../INSTALL_SANDBOX.md) for detailed installation instructions
- Check out [example skills](./.skills/) in the repository
- Learn about [security features](../README.md#core-innovation-native-system-level-security-sandbox)
- Explore [MCP integration](../README.md) for Claude Desktop

## Troubleshooting

### Binary not found

If `skilllite install` fails, you can manually download from:
https://github.com/EXboys/skilllite/releases

### Platform not supported

Currently supported:
- macOS (x64, ARM64)
- Linux (x64, ARM64)

Windows support coming soon.

### Auto-install disabled

If you want to disable auto-installation:

```python
from skilllite.sandbox.skillbox import ensure_installed

# This will raise an error if not installed
binary_path = ensure_installed(auto_install=False)
```

