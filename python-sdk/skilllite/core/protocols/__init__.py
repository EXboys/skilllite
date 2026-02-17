"""
SkillLite Protocols — unified types for adapters (LangChain, LlamaIndex, MCP).
"""

import asyncio
from dataclasses import dataclass
from typing import Callable, Optional

from ..security import SecurityScanResult

ConfirmationCallback = Callable[[str, str], bool]
AsyncConfirmationCallback = Callable[[str, str], "asyncio.Future[bool]"]


@dataclass
class ExecutionOptions:
    """Options for skill execution — shared across adapters."""
    sandbox_level: int = 3
    allow_network: bool = False
    timeout: Optional[int] = None
    confirmation_callback: Optional[ConfirmationCallback] = None
    async_confirmation_callback: Optional[AsyncConfirmationCallback] = None


__all__ = [
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
    "ExecutionOptions",
]
