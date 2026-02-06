"""
SkillLite Protocols - Unified type definitions and interfaces.

This module provides the core types and protocols that all adapters
(LangChain, LlamaIndex, MCP, etc.) should use. This ensures consistency
across different framework integrations.

Key Components:
- SecurityScanResult: Unified security scan result
- ConfirmationCallback: Type alias for confirmation callbacks
- AdapterProtocol: Abstract base for all adapters
"""

from .types import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
    ExecutionOptions,
)

__all__ = [
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
    "ExecutionOptions",
]

