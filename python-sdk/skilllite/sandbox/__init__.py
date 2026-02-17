"""
Sandbox module - provides sandboxed execution environments.

This module abstracts different sandbox implementations, with skilllite
(Rust-based sandbox) as the primary implementation.
"""

from .context import (
    SandboxExecutor,
    ExecutionResult,
    SandboxConfig,
    DEFAULT_EXECUTION_TIMEOUT,
    DEFAULT_MAX_MEMORY_MB,
    DEFAULT_SANDBOX_LEVEL,
    DEFAULT_ALLOW_NETWORK,
    DEFAULT_ENABLE_SANDBOX,
)
from .core import install, uninstall, find_binary, ensure_installed
from .utils import (
    convert_json_to_cli_args,
    extract_json_from_output,
    format_sandbox_error,
    DEFAULT_POSITIONAL_KEYS,
)

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
    # Skillbox binary management
    "install",
    "uninstall", 
    "find_binary",
    "ensure_installed",
    # Utilities
    "convert_json_to_cli_args",
    "extract_json_from_output",
    "format_sandbox_error",
    "DEFAULT_POSITIONAL_KEYS",
]
