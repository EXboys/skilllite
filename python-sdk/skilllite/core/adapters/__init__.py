"""
SkillLite Adapters - Framework adapters for LangChain, LlamaIndex, etc.

This module provides adapters for integrating SkillLite with popular AI frameworks:
- LangChain: SkillLiteTool, SkillLiteToolkit
- LlamaIndex: SkillLiteToolSpec
- BaseAdapter: Abstract base class for custom adapters

All adapters inherit from BaseAdapter and share common types:
- SecurityScanResult: Contains scan results with severity counts
- ConfirmationCallback: Type alias for (report: str, scan_id: str) -> bool
- AsyncConfirmationCallback: Async version of ConfirmationCallback

Usage:
    # LangChain (requires: pip install skilllite[langchain])
    from skilllite.core.adapters.langchain import SkillLiteTool, SkillLiteToolkit

    # LlamaIndex (requires: pip install skilllite[llamaindex])
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

    # Shared types (no additional dependencies required)
    from skilllite.core.adapters import SecurityScanResult, ConfirmationCallback

    # Custom adapter (inherit from BaseAdapter)
    from skilllite.core.adapters import BaseAdapter

    # Security confirmation callback
    def confirm(report: str, scan_id: str) -> bool:
        print(report)
        return input("Continue? [y/N]: ").lower() == 'y'

    toolkit = SkillLiteToolkit.from_manager(
        manager, sandbox_level=3, confirmation_callback=confirm
    )
"""

# Import shared types from security module
from ..security import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
    ExecutionOptions,
)

# Import BaseAdapter - always available (no external dependencies)
from .base import BaseAdapter

__all__ = [
    # Framework adapters (lazy-loaded, require their respective dependencies)
    "SkillLiteTool",
    "SkillLiteToolkit",
    "SkillLiteToolSpec",
    # Base class for custom adapters (always available)
    "BaseAdapter",
    # Shared types (always available)
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
    "ExecutionOptions",
]


def __getattr__(name: str):
    """Lazy import framework-specific adapters to avoid requiring all dependencies."""
    if name in ("SkillLiteTool", "SkillLiteToolkit"):
        try:
            from .langchain import SkillLiteTool, SkillLiteToolkit
            return {"SkillLiteTool": SkillLiteTool, "SkillLiteToolkit": SkillLiteToolkit}[name]
        except ImportError as e:
            raise ImportError(
                f"LangChain adapter requires langchain-core. "
                f"Install with: pip install skilllite[langchain]\n"
                f"Original error: {e}"
            ) from e

    if name == "SkillLiteToolSpec":
        try:
            from .llamaindex import SkillLiteToolSpec
            return SkillLiteToolSpec
        except ImportError as e:
            raise ImportError(
                f"LlamaIndex adapter requires llama-index. "
                f"Install with: pip install skilllite[llamaindex]\n"
                f"Original error: {e}"
            ) from e

    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")

