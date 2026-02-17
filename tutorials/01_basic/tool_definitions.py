"""
View Tool Definitions (Claude and OpenAI formats)

Quick Start:
  python tool_definitions.py

Uses list_tools RPC (no SkillManager).
"""

import sys
import os
import json
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite.sandbox.core import find_binary
from skilllite.sandbox.core.ipc_client import SkillboxIPCClientPool

skills_dir = str(Path(__file__).parent.resolve() / "../../.skills")
binary = find_binary()
if not binary:
    print("❌ skilllite not found. Run: skilllite install")
    exit(1)

pool = SkillboxIPCClientPool(binary_path=binary)
tools_openai = pool.list_tools(skills_dir, format="openai")
tools_claude = pool.list_tools(skills_dir, format="claude")
pool.close()

if not tools_openai:
    print("❌ No skills found")
    exit(1)

# OpenAI format (first tool)
print("=== OpenAI Format ===")
print(json.dumps(tools_openai[0], indent=2, ensure_ascii=False))

print("\n=== Claude Format ===")
print(json.dumps(tools_claude[0], indent=2, ensure_ascii=False))
