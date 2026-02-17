"""
Skillbox sandbox implementation.

This module provides the Rust-based skillbox binary management and IPC client.
Execution is handled by ipc_executor (sandbox/ipc_executor.py).
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

__all__ = [
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
]
