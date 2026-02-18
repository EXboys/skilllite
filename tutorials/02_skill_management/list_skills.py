"""
Skill Management: List and execute skills

Quick Start:
  python list_skills.py

Uses skilllite list + run via subprocess (no SkillManager).
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

# ========== List All Skills ==========

result = subprocess.run(
    [binary, "list", "-s", skills_dir, "--json"],
    capture_output=True, text=True, timeout=30,
)
if result.returncode != 0:
    print("❌ Failed to list skills")
    exit(1)

skills = json.loads(result.stdout) if result.stdout.strip() else []
print(f"Found {len(skills)} skills:\n")

for skill in skills:
    print(f"📌 {skill.get('name', '')}")
    print(f"   Description: {skill.get('description', '')}")
    print(f"   Language: {skill.get('language', '')}")
    print(f"   Entry Point: {skill.get('entry_point', '')}")
    print()

# ========== Execute a Skill ==========

if skills:
    # Pick first runnable skill (has entry_point)
    runnable = [s for s in skills if s.get("entry_point")]
    skill = runnable[0] if runnable else skills[0]
    skill_path = skill.get("path", "")
    skill_name = skill.get("name", "unknown")
    print(f"Executing skill: {skill_name}")
    print("-" * 40)

    run_result = subprocess.run(
        [binary, "run", skill_path, "{}"],
        capture_output=True, text=True, timeout=60,
    )

    if run_result.returncode == 0:
        print(f"✅ Output: {run_result.stdout.strip()}")
    else:
        print(f"❌ Error: {run_result.stderr or run_result.stdout}")
