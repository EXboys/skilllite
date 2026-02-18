"""
Minimal Example: One-line execution

Quick Start:
1. Copy .env.example → .env and configure your API key
2. skilllite init  # optional
3. python hello_world.py
"""

import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import chat

# One line to run (uses .env for API config)
result = chat("Write a poem about Python", skills_dir="../../.skills")
print(result)
