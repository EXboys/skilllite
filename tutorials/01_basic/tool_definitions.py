"""
View Tool Definitions (Claude and OpenAI formats)

Quick Start:
1. python tool_definitions.py
"""

import sys
import os
import json
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillManager

# Initialize
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

# Get tool definitions
tools = manager.get_tools()
if not tools:
    print("❌ No skills found")
    exit(1)

tool = tools[0]

# Claude format
print("=== Claude Format ===")
print(json.dumps(tool.to_claude_format(), indent=2, ensure_ascii=False))

print("\n=== OpenAI Format ===")
print(json.dumps(tool.to_openai_format(), indent=2, ensure_ascii=False))
