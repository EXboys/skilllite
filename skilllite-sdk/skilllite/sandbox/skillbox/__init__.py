"""
Skillbox sandbox implementation.

This module provides the Rust-based skillbox sandbox executor,
including binary management and execution logic.
"""

from .binary import (
    BINARY_VERSION,
    BINARY_NAME,
    get_install_dir,
    get_binary_path,
    get_version_file,
    get_platform,
    get_download_url,
    is_installed,
    get_installed_version,
    needs_update,
    install,
    uninstall,
    find_binary,
    ensure_installed,
    invalidate_binary_cache,
    check_binary_version,
)
from .executor import SkillboxExecutor

__all__ = [
    # Binary management
    "BINARY_VERSION",
    "BINARY_NAME",
    "get_install_dir",
    "get_binary_path",
    "get_version_file",
    "get_platform",
    "get_download_url",
    "is_installed",
    "get_installed_version",
    "needs_update",
    "install",
    "uninstall",
    "find_binary",
    "ensure_installed",
    "invalidate_binary_cache",
    "check_binary_version",
    # Executor
    "SkillboxExecutor",
]
