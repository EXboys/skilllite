"""
Skill Management: List, inspect, and execute skills

Quick Start:
1. python list_skills.py - List all skills
2. python execute_skill.py - Execute a specific skill
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillManager

# Initialize
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

# ========== List All Skills ==========

skills = manager.list_skills()
print(f"Found {len(skills)} skills:\n")

for skill in skills:
    print(f"📌 {skill.name}")
    print(f"   Description: {skill.description}")
    print(f"   Language: {skill.language}")
    print(f"   Entry Point: {skill.metadata.entry_point}")
    print()

# ========== Execute a Skill ==========

if skills:
    skill = skills[0]
    print(f"Executing skill: {skill.name}")
    print("-" * 40)

    # Prepare parameters
    input_data = {}

    # Execute
    result = manager.execute(skill.name, input_data)

    if result.success:
        print(f"✅ Output: {result.output}")
    else:
        print(f"❌ Error: {result.error}")
