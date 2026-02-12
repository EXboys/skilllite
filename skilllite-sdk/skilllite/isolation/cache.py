"""
Cache utilities for isolated skill environments.

Provides cache directory and key computation.
Mirrors skillbox/src/env logic.
"""

import hashlib
import os
import platform
from pathlib import Path


def get_cache_dir() -> Path:
    """Get the environment cache directory (mirrors Rust get_cache_dir)."""
    env_override = os.environ.get("AGENTSKILL_CACHE_DIR")
    if env_override:
        return Path(env_override) / "agentskill" / "envs"
    system = platform.system()
    if system == "Darwin":
        base = Path.home() / "Library" / "Caches"
    elif system == "Linux":
        xdg = os.environ.get("XDG_CACHE_HOME")
        base = Path(xdg) if xdg else Path.home() / ".cache"
    else:
        base = Path.home() / ".cache"
    return base / "agentskill" / "envs"


def compute_packages_hash(packages: list[str]) -> str:
    """Compute SHA-256 hash from package list (mirrors Rust compute_packages_hash).

    The list is **sorted** before hashing so that different orderings of the
    same packages always produce the same hash.
    """
    h = hashlib.sha256()
    for pkg in sorted(packages):
        h.update(pkg.encode())
        h.update(b"\n")
    return h.hexdigest()


def get_cache_key(language: str, content_hash: str) -> str:
    """Get cache key for a dependency configuration (mirrors Rust get_cache_key).

    Uses the same prefix mapping as Rust: python -> "py", node -> "node".
    """
    prefix_map = {"python": "py", "node": "node"}
    prefix = prefix_map.get(language, language)
    if not content_hash:
        return f"{prefix}-none"
    return f"{prefix}-{content_hash[:16]}"
