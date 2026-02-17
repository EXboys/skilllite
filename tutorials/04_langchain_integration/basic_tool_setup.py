"""
LangChain Integration: Using SkillLite as tools

Prerequisites:
  pip install skilllite[langchain]

Example: Convert SkillLite skills to LangChain tools (RPC-based, no SkillManager)
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

skills_dir = str(Path(__file__).parent / "../../.skills")

# ========== Method 1: Using SkillLiteToolkit.from_skills_dir (Recommended) ==========

try:
    from skilllite.core.adapters.langchain import SkillLiteToolkit

    # Create LangChain tools via RPC (no SkillManager)
    tools = SkillLiteToolkit.from_skills_dir(skills_dir)

    # Or with options:
    # tools = SkillLiteToolkit.from_skills_dir(
    #     skills_dir,
    #     skill_names=["calculator", "web_search"],
    #     allow_network=True,
    #     timeout=60,
    # )

    print(f"✅ Created {len(tools)} LangChain tools using SkillLiteToolkit.from_skills_dir")

    for tool in tools:
        print(f"   - {tool.name}: {tool.description}")

except ImportError as e:
    print("❌ LangChain not installed. Install with: pip install skilllite[langchain]")
    tools = []

# ========== Method 2: OpenAI format via list_tools ==========

try:
    from skilllite.sandbox.core import find_binary
    from skilllite.sandbox.core.ipc_client import SkillboxIPCClientPool

    binary = find_binary()
    if binary:
        pool = SkillboxIPCClientPool(binary_path=binary)
        openai_tools = pool.list_tools(skills_dir)
        pool.close()
        print(f"\n📋 Also available: {len(openai_tools)} tools in OpenAI format")
    else:
        print("\n📋 skilllite binary not found. Run: skilllite install")
except Exception:
    pass
