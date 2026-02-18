"""
Direct Skill Execution (without LLM)

Quick Start:
  python direct_execution.py

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

# List skills via skilllite list --json
result = subprocess.run(
    [binary, "list", "-s", skills_dir, "--json"],
    capture_output=True, text=True, timeout=30,
)
if result.returncode != 0 or not result.stdout.strip():
    print("❌ No skills found. Please create skills in the .skills directory")
    exit(1)

skills = json.loads(result.stdout)
# Pick first skill with entry_point (runnable via "run")
runnable = [s for s in skills if s.get("entry_point") or s.get("is_bash_tool")]
skill = runnable[0] if runnable else skills[0]
skill_path = skill.get("path", "")
skill_name = skill.get("name", "unknown")

if skill.get("is_bash_tool"):
    # Bash-tool: use bash command
    run_result = subprocess.run(
        [binary, "bash", skill_path, "agent-browser --help"],
        capture_output=True, text=True, timeout=30,
    )
else:
    run_result = subprocess.run(
        [binary, "run", skill_path, "{}"],
        capture_output=True, text=True, timeout=60,
    )

if run_result.returncode == 0:
    out = run_result.stdout.strip()
    try:
        data = json.loads(out)
        print(f"✅ {skill_name}: {data}")
    except json.JSONDecodeError:
        print(f"✅ {skill_name}: {out}")
else:
    print(f"❌ {skill_name}: {run_result.stderr or run_result.stdout}")
