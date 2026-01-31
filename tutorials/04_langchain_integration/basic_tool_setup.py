"""
LangChain Integration: Using SkillLite as tools

Prerequisites:
  pip install langchain langchain-openai

Example: Convert SkillLite skills to LangChain tools
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager

# ========== Basic Setup ==========

# Initialize SkillManager
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

# Get OpenAI format tool definitions (used by LangChain)
tools = manager.get_tools()

# Tools are ready to be passed to LangChain
# Example:
#   from langchain.agents import create_openai_tools_agent
#   agent = create_openai_tools_agent(llm, tools, prompt)

print(f"✅ Prepared {len(tools)} tools for LangChain")
