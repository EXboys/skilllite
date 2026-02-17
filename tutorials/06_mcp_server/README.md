# 06. MCP Server

## Core Examples

### mcp_client_test.py
Test SkillLite MCP server functionality

## Claude Desktop Configuration

Edit `~/.config/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "skilllite": {
      "command": "skilllite",
      "args": ["mcp"],
      "env": {
        "SKILLBOX_SANDBOX_LEVEL": "3",
        "SKILLLITE_SKILLS_DIR": "/path/to/skills"
      }
    }
  }
}
```

> **Note**: Use `skilllite mcp` (Rust binary). Run `skilllite install` first if needed.

Restart Claude Desktop to use SkillLite tools in conversations.

## MCP Protocol

SkillLite implements two core tools:

1. **scan_code** - Scan code for security issues
   - Identify potential security problems
   - Return risk level (High/Medium/Low)

2. **execute_code** - Execute code
   - Safely execute in sandbox
   - Supports Python and other languages

## Prerequisites

```bash
pip install mcp
```

## Next Steps

- Back to [04. LangChain](../04_langchain_integration/README.md)
- Back to [05. LlamaIndex](../05_llamaindex_integration/README.md)
