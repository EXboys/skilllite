"""
Package whitelist for skill dependency resolution.

Single source of truth: packages_whitelist.json (same directory).
Synced with skillbox/src/skill/deps.rs. Dependency resolution is delegated to skillbox.
"""

import json
from pathlib import Path
from typing import List, Tuple

_WHITELIST_PATH = Path(__file__).parent / "packages_whitelist.json"
_CACHE: dict | None = None


def _load_whitelist() -> dict:
    """Load whitelist from JSON. Cached for performance."""
    global _CACHE
    if _CACHE is None:
        with open(_WHITELIST_PATH, encoding="utf-8") as f:
            _CACHE = json.load(f)
    return _CACHE


def get_python_packages() -> List[str]:
    """Return list of known Python package names."""
    data = _load_whitelist()
    return list(data.get("python", []))


def get_python_aliases() -> List[str]:
    """Return Python package name aliases (e.g. opencv-python -> cv2)."""
    data = _load_whitelist()
    return list(data.get("python_aliases", []))


def get_node_packages() -> List[str]:
    """Return list of known Node.js package names."""
    data = _load_whitelist()
    return list(data.get("node", []))


def get_all_packages() -> Tuple[List[str], List[str]]:
    """Return (python_packages, node_packages)."""
    return get_python_packages(), get_node_packages()
