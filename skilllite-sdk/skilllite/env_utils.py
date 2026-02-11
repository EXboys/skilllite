"""
Shared environment utilities for skill execution.

This module provides ensure_skill_python() used by UnifiedExecutor
to resolve the Python executable for a skill with dependencies.
Extracted to avoid code duplication.
"""

import os
import sys
from pathlib import Path


def ensure_skill_python(skill_dir: Path) -> str:
    """Get Python executable with dependencies installed if needed.

    If the skill has dependencies (from ``.skilllite.lock`` or the
    ``compatibility`` field in SKILL.md), ensures a virtual environment
    exists with those deps installed and returns the venv's python path.
    Otherwise returns ``sys.executable``.

    Used by UnifiedExecutor for Level 1/2 direct Python execution
    direct execution, so that dependency management works without
    requiring ``skilllite init``.
    """
    try:
        from .core.metadata import parse_skill_metadata
        from .cli.init import (
            parse_compatibility_for_packages,
            _get_cache_dir,
            _compute_packages_hash,
            _get_cache_key,
            _ensure_python_env,
        )
    except ImportError:
        return sys.executable

    try:
        metadata = parse_skill_metadata(skill_dir)
    except Exception:
        return sys.executable

    # Prefer resolved_packages from .skilllite.lock, fallback to whitelist parsing
    packages = metadata.resolved_packages
    if packages is None:
        packages = parse_compatibility_for_packages(metadata.compatibility)

    if not packages:
        return sys.executable

    # Compute cache key and ensure venv exists
    language = metadata.language or "python"
    content_hash = _compute_packages_hash(packages)
    cache_key = _get_cache_key(language, content_hash)
    cache_dir = _get_cache_dir()
    cache_dir.mkdir(parents=True, exist_ok=True)
    env_path = cache_dir / cache_key

    # Create venv and install packages (idempotent — skips if marker exists)
    _ensure_python_env(env_path, packages)

    # Return venv's python executable
    python = (
        env_path / "Scripts" / "python"
        if os.name == "nt"
        else env_path / "bin" / "python"
    )
    return str(python) if python.exists() else sys.executable
