"""
SkillLite - A lightweight Skills secure execution engine.

pip install skilllite → full CLI + sandbox API

- CLI: skilllite chat/add/list/mcp/... (all commands via bundled binary)
- API (Python ↔ binary bridge): scan_code, execute_code, chat
"""

from .api import scan_code, execute_code, chat, run_skill
from .binary import get_binary

__version__ = "0.1.9"
__all__ = ["scan_code", "execute_code", "chat", "run_skill", "get_binary"]
