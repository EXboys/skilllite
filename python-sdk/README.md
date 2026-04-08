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
from skilllite import scan_code, execute_code, chat, run_skill

# Sandbox: security scan + execute (IDE/MCP integration)
result = scan_code("python", "print(1+1)")
result = execute_code("python", "print(sum(range(101)))")

# Direct skill execution
result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')

# Agent chat (single-shot, hides binary CLI)
result = chat("帮我分析这个项目", skills_dir=".skills", stream=True)
# result["success"], result["exit_code"]
```

## Artifacts (run-scoped blobs over HTTP)

Use the same API as `docs/openapi/artifact-store-http-v1.yaml`. No extra pip dependencies (stdlib `urllib`).

```python
from skilllite import artifact_put, artifact_get

artifact_put("http://127.0.0.1:8080", "my-run-id", "outputs/result.json", b'{"ok": true}')
data = artifact_get("http://127.0.0.1:8080", "my-run-id", "outputs/result.json")
```

Serve locally with the main CLI (subcommand is in the default binary; **must** allow bind explicitly):

```bash
cargo build -p skilllite --bin skilllite
SKILLLITE_ARTIFACT_SERVE_ALLOW=1 ./target/debug/skilllite artifact-serve --dir /tmp/art --bind 127.0.0.1:8080
```

## Build from source

```bash
./scripts/build_wheels.sh
```
