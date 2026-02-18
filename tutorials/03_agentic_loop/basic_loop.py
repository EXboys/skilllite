"""
Agentic Loop: Multi-turn conversations and tool calls

Quick Start:
  python basic_loop.py

Uses chat() API — bridges to skilllite chat (Rust binary).
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import chat

# ========== Using SkillLite chat API ==========

def skilllite_agent(user_request: str):
    """Run agent via chat() API (skilllite binary)."""
    skills_dir = str(Path(__file__).parent / "../../.skills")
    return chat(user_request, skills_dir=skills_dir)


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me complete a task"
    result = skilllite_agent(request)
    print(f"Final result: {result}")
