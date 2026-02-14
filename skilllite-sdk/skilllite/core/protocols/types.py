"""
Unified type definitions for SkillLite adapters.

This module contains shared types that are used across different
framework adapters (LangChain, LlamaIndex, MCP, etc.).

SecurityScanResult is defined in core/security.py (Single Source of Truth)
and re-exported here for backward compatibility.
"""

import asyncio
from dataclasses import dataclass
from typing import Callable, Optional

# Re-export SecurityScanResult from its canonical location
from ..security import SecurityScanResult


# Type aliases for confirmation callbacks
ConfirmationCallback = Callable[[str, str], bool]
"""Synchronous confirmation callback: (security_report: str, scan_id: str) -> bool"""

AsyncConfirmationCallback = Callable[[str, str], "asyncio.Future[bool]"]
"""Asynchronous confirmation callback: (security_report: str, scan_id: str) -> Future[bool]"""


@dataclass
class ExecutionOptions:
    """Options for skill execution - shared across all adapters."""

    sandbox_level: int = 3
    """Sandbox security level (1/2/3). Default: 3 (full security)"""

    allow_network: bool = False
    """Whether to allow network access during execution."""

    timeout: Optional[int] = None
    """Execution timeout in seconds. None means use default."""

    confirmation_callback: Optional[ConfirmationCallback] = None
    """Callback for security confirmation (sync)."""

    async_confirmation_callback: Optional[AsyncConfirmationCallback] = None
    """Callback for security confirmation (async)."""


__all__ = [
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
    "ExecutionOptions",
]

