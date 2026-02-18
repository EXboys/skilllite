"""
View Tool Definitions (Claude and OpenAI formats)

Quick Start:
  python tool_definitions.py

Uses skilllite list-tools (CLI subprocess).
"""

import sys
import os
import json
import subprocess
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import get_binary

skills_dir = str(Path(__file__).parent.resolve() / "../../.skills")
binary = get_binary()
if not binary:
    print("❌ skilllite not found. Run: pip install skilllite")
    exit(1)

def list_tools(fmt: str) -> list:
    result = subprocess.run(
        [binary, "list-tools", "-s", skills_dir, "--format", fmt],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        return []
    data = json.loads(result.stdout)
    return data.get("tools", [])

tools_openai = list_tools("openai")
tools_claude = list_tools("claude")

if not tools_openai:
    print("❌ No skills found")
    exit(1)

# OpenAI format (first tool)
print("=== OpenAI Format ===")
print(json.dumps(tools_openai[0], indent=2, ensure_ascii=False))

print("\n=== Claude Format ===")
print(json.dumps(tools_claude[0], indent=2, ensure_ascii=False))
