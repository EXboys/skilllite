"""
SkillLite - A lightweight Skills secure execution engine.

pip install skilllite → full CLI + sandbox API

- CLI: skilllite chat/add/list/mcp/... (all commands via bundled binary)
- API (Python ↔ binary bridge): scan_code, execute_code, chat
"""

from .api import chat, execute_code, run_skill, scan_code
from .binary import get_binary

__version__ = "0.1.16"
__all__ = ["scan_code", "execute_code", "chat", "run_skill", "get_binary"]
