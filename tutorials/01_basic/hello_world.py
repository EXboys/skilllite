"""
Minimal Example: One-line execution

Quick Start:
1. Copy .env.example → .env and configure your API key
2. python hello_world.py
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillRunner

# Initialize (automatically reads .env)
runner = SkillRunner()

# One line to run
result = runner.run("Write a poem about Python")
print(result)
