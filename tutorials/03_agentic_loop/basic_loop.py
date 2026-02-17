"""
Agentic Loop: Multi-turn conversations and tool calls

Quick Start:
  python basic_loop.py

Uses SkillRunner (agent_chat RPC) — no SkillManager.
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillRunner

# ========== Using SkillLite Built-in Agent (agent_chat RPC) ==========

def skilllite_agent(user_request: str):
    """Run agent via skilllite agent-rpc (Rust)."""
    runner = SkillRunner(skills_dir=str(Path(__file__).parent / "../../.skills"))
    return runner.run(user_request)


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me complete a task"
    result = skilllite_agent(request)
    print(f"Final result: {result}")
