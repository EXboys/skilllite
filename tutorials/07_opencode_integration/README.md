# 07. OpenCode Integration

Integrate SkillLite as an MCP tool server for [OpenCode](https://github.com/anomalyco/opencode) - the open source AI coding agent.

## Overview

OpenCode supports MCP (Model Context Protocol) for extending its capabilities. By integrating SkillLite, OpenCode gains:

- **Secure Code Execution**: Run Python/JavaScript/Bash in a sandboxed environment
- **Security Scanning**: Scan code for vulnerabilities before execution
- **Custom Skills**: Execute your custom skills through OpenCode

## Prerequisites

```bash
# 1. Install OpenCode
brew install anomalyco/tap/opencode  # macOS
# or
npm i -g opencode-ai@latest

# 2. Install SkillLite with MCP support
pip install skilllite[mcp]

# 3. Install skillbox sandbox
skilllite install
```

## Configuration

### Option 1: Project-level Configuration (Recommended)

Create `.opencode/config.json` in your project root:

```json
{
  "mcp": {
    "servers": {
      "skilllite": {
        "command": "python",
        "args": ["-m", "skilllite.mcp.server"],
        "env": {
          "SKILLBOX_SANDBOX_LEVEL": "3",
          "SKILLLITE_SKILLS_DIR": "./skills"
        }
      }
    }
  }
}
```

### Option 2: Global Configuration

Create `~/.config/opencode/config.json`:

```json
{
  "mcp": {
    "servers": {
      "skilllite": {
        "command": "skilllite",
        "args": ["mcp"],
        "env": {
          "SKILLBOX_SANDBOX_LEVEL": "3"
        }
      }
    }
  }
}
```

## Available Tools

Once configured, OpenCode will have access to these tools:

### 1. `scan_code`
Scan code for security issues before execution.

```
Arguments:
  - language: "python" | "javascript" | "bash"
  - code: The code to scan
```

### 2. `execute_code`
Execute code in a secure sandbox.

```
Arguments:
  - language: "python" | "javascript" | "bash"
  - code: The code to execute
  - confirmed: boolean (required if security issues found)
  - scan_id: string (from scan_code result)
  - sandbox_level: 1 | 2 | 3 (optional)
```

## Sandbox Levels

| Level | Description | Use Case |
|-------|-------------|----------|
| 1 | No sandbox | Trusted code, full system access |
| 2 | Sandbox only | Isolated execution, no security scan |
| 3 | Sandbox + Scan | Maximum security (default) |

## Usage Examples

After configuration, start OpenCode in your project:

```bash
cd your-project
opencode
```

Then ask OpenCode to execute code:

```
> Run this Python code safely: print("Hello from SkillLite!")

> Scan this code for security issues: import os; os.system("rm -rf /")

> Execute a fibonacci calculation in the sandbox
```

## Verification

Test that the integration works:

```bash
# Start OpenCode and check available tools
opencode

# In OpenCode, type:
> What MCP tools do you have available?
```

You should see `scan_code` and `execute_code` in the list.

## Troubleshooting

### MCP Server Not Starting

```bash
# Test MCP server manually
python -m skilllite.mcp.server

# Check skillbox is installed
skilllite status
```

### Permission Errors

```bash
# Ensure skillbox binary is executable
chmod +x ~/.skilllite/bin/skillbox
```

### Debug Mode

Set environment variable for verbose logging:

```json
{
  "mcp": {
    "servers": {
      "skilllite": {
        "command": "python",
        "args": ["-m", "skilllite.mcp.server"],
        "env": {
          "SKILLBOX_DEBUG": "1"
        }
      }
    }
  }
}
```

## Security Workflow

OpenCode + SkillLite follows a secure execution pattern:

1. **Scan First**: OpenCode calls `scan_code` to check for issues
2. **Review**: Security report is shown to the user
3. **Confirm**: If issues found, user must confirm execution
4. **Execute**: Code runs in isolated sandbox

## Next Steps

- [06. MCP Server](../06_mcp_server/README.md) - More MCP details
- [OpenCode Documentation](https://opencode.ai/docs)
- [MCP Protocol Specification](https://modelcontextprotocol.io/)

