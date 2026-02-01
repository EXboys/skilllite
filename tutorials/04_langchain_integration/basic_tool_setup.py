"""
LangChain Integration: Using SkillLite as tools

Prerequisites:
  pip install skilllite[langchain]
  # or: pip install langchain-core

Example: Convert SkillLite skills to LangChain tools using the official adapter
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager

# ========== Method 1: Using SkillLiteToolkit (Recommended) ==========

# Initialize SkillManager
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

try:
    from skilllite.core.adapters.langchain import SkillLiteToolkit, SkillLiteTool

    # Create LangChain tools from all skills
    tools = SkillLiteToolkit.from_manager(manager)

    # Or with options:
    # tools = SkillLiteToolkit.from_manager(
    #     manager,
    #     skill_names=["calculator", "web_search"],  # Only specific skills
    #     allow_network=True,                         # Allow network access
    #     timeout=60                                  # Execution timeout
    # )

    print(f"✅ Created {len(tools)} LangChain tools using SkillLiteToolkit")

    # Each tool is a LangChain BaseTool that can be used with agents
    for tool in tools:
        print(f"   - {tool.name}: {tool.description}")

except ImportError:
    print("❌ LangChain not installed. Install with: pip install skilllite[langchain]")
    tools = []

# ========== Method 2: Using OpenAI format (Legacy) ==========

# Get OpenAI format tool definitions (for manual agent loops)
openai_tools = manager.get_tools()
print(f"\n📋 Also available: {len(openai_tools)} tools in OpenAI format")
