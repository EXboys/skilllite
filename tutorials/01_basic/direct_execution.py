"""
Direct Skill Execution (without LLM)

Quick Start:
1. python direct_execution.py
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager

# Initialize
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

# List all skills
skills = manager.list_skills()
if not skills:
    print("❌ No skills found. Please create skills in the .skills directory")
    exit(1)

# Execute the first skill
skill = skills[0]
result = manager.execute(skill.name, {})

if result.success:
    print(f"✅ {skill.name}: {result.output}")
else:
    print(f"❌ {skill.name}: {result.error}")
