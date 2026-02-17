# SkillLite

A lightweight Skills secure execution engine. **One package: full CLI + sandbox API.**

```bash
pip install skilllite
```

## CLI (full capability)

```bash
skilllite chat              # Interactive chat with LLM
skilllite add owner/repo    # Add skills from GitHub
skilllite list              # List installed skills
skilllite mcp               # Start MCP server (for Cursor/Claude)
skilllite run/exec/bash     # Execute skills
skilllite init-cursor       # Initialize Cursor IDE integration
# ... and more
```

## API (Python ↔ binary bridge)

```python
from skilllite import scan_code, execute_code, chat

# Sandbox: security scan + execute (IDE/MCP integration)
result = scan_code("python", "print(1+1)")
result = execute_code("python", "print(sum(range(101)))")

# Agent chat (single-shot, hides binary CLI)
result = chat("帮我分析这个项目", skills_dir=".skills", stream=True)
# result["success"], result["exit_code"]
```

## Build from source

```bash
./scripts/build_wheels.sh
```
