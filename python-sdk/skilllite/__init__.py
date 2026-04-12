"""
SkillLite - A lightweight Skills secure execution engine.

pip install skilllite → full CLI + sandbox API

- CLI: skilllite chat/add/list/mcp/... (all commands via bundled binary)
- API (Python ↔ binary bridge): scan_code, execute_code, chat
- Artifacts (HTTP, stdlib only): artifact_put, artifact_get — OpenAPI v1 client
"""

from .api import chat, execute_code, run_skill, scan_code
from .artifacts import ArtifactHttpError, artifact_get, artifact_put, parse_listen_line
from .binary import get_binary

__version__ = "0.1.24"
__all__ = [
    "scan_code",
    "execute_code",
    "chat",
    "run_skill",
    "get_binary",
    "artifact_put",
    "artifact_get",
    "ArtifactHttpError",
    "parse_listen_line",
]
