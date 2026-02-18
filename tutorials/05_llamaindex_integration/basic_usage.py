"""
LlamaIndex + SkillLite Integration

Note: SkillLiteToolSpec has been removed from the main skilllite package.
For LlamaIndex integration, consider:
  1. Using langchain-skilllite with LangChain (see 04_langchain_integration)
  2. Using SkillLite MCP Server (see 06_mcp_server)
  3. Calling skilllite CLI directly via subprocess

Example - Direct CLI call:
"""
import subprocess
from pathlib import Path
import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import get_binary

skills_dir = str(Path(__file__).parent / "../../.skills")
binary = get_binary()
if not binary:
    print("❌ skilllite not found. Run: pip install skilllite")
    sys.exit(1)

# List skills
result = subprocess.run(
    [binary, "list", "-s", skills_dir, "--json"],
    capture_output=True, text=True, timeout=30,
)
if result.returncode == 0 and result.stdout.strip():
    import json
    skills = json.loads(result.stdout)
    print(f"Available skills: {[s.get('name') for s in skills]}")
else:
    print("No skills found. Add skills with: skilllite add <source>")
