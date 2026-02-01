"""
SkillLite Adapters - Framework adapters for LangChain, LlamaIndex, etc.

This module provides adapters for integrating SkillLite with popular AI frameworks:
- LangChain: SkillLiteTool, SkillLiteToolkit
- LlamaIndex: SkillLiteToolSpec

Both adapters support sandbox security confirmation (sandbox_level=3):
- SecurityScanResult: Contains scan results with severity counts
- ConfirmationCallback: Type alias for (report: str, scan_id: str) -> bool

Usage:
    # LangChain (requires: pip install skilllite[langchain])
    from skilllite.core.adapters.langchain import SkillLiteTool, SkillLiteToolkit

    # LlamaIndex (requires: pip install skilllite[llamaindex])
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

    # Security confirmation callback
    def confirm(report: str, scan_id: str) -> bool:
        print(report)
        return input("Continue? [y/N]: ").lower() == 'y'

    toolkit = SkillLiteToolkit.from_manager(
        manager, sandbox_level=3, confirmation_callback=confirm
    )
"""

__all__ = [
    "SkillLiteTool",
    "SkillLiteToolkit",
    "SkillLiteToolSpec",
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
]


def __getattr__(name: str):
    """Lazy import to avoid requiring all dependencies at import time."""
    if name in ("SkillLiteTool", "SkillLiteToolkit", "SecurityScanResult",
                "ConfirmationCallback", "AsyncConfirmationCallback"):
        try:
            from .langchain import (
                SkillLiteTool, SkillLiteToolkit, SecurityScanResult,
                ConfirmationCallback, AsyncConfirmationCallback
            )
            return {
                "SkillLiteTool": SkillLiteTool,
                "SkillLiteToolkit": SkillLiteToolkit,
                "SecurityScanResult": SecurityScanResult,
                "ConfirmationCallback": ConfirmationCallback,
                "AsyncConfirmationCallback": AsyncConfirmationCallback,
            }[name]
        except ImportError as e:
            raise ImportError(
                f"LangChain adapter requires langchain. "
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

