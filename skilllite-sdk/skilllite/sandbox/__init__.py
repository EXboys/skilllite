"""
Sandbox module - provides sandboxed execution environments.

This module abstracts different sandbox implementations, with skillbox
(Rust-based sandbox) as the primary implementation.
"""

from .base import SandboxExecutor, ExecutionResult
from .config import (
    SandboxConfig,
    DEFAULT_EXECUTION_TIMEOUT,
    DEFAULT_MAX_MEMORY_MB,
    DEFAULT_SANDBOX_LEVEL,
    DEFAULT_ALLOW_NETWORK,
    DEFAULT_ENABLE_SANDBOX,
)
from .skillbox import SkillboxExecutor, install, uninstall, find_binary, ensure_installed

__all__ = [
    # Base classes
    "SandboxExecutor",
    "ExecutionResult",
    # Configuration
    "SandboxConfig",
    "DEFAULT_EXECUTION_TIMEOUT",
    "DEFAULT_MAX_MEMORY_MB",
    "DEFAULT_SANDBOX_LEVEL",
    "DEFAULT_ALLOW_NETWORK",
    "DEFAULT_ENABLE_SANDBOX",
    # Skillbox implementation
    "SkillboxExecutor",
    "install",
    "uninstall", 
    "find_binary",
    "ensure_installed",
]
