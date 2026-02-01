"""
SkillLite Adapters - Framework adapters for LangChain, LlamaIndex, etc.

This module provides adapters for integrating SkillLite with popular AI frameworks:
- LangChain: SkillLiteTool, SkillLiteToolkit
- LlamaIndex: SkillLiteToolSpec

Usage:
    # LangChain (requires: pip install skilllite[langchain])
    from skilllite.core.adapters.langchain import SkillLiteTool, SkillLiteToolkit
    
    # LlamaIndex (requires: pip install skilllite[llamaindex])
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
"""

__all__ = [
    "SkillLiteTool",
    "SkillLiteToolkit",
    "SkillLiteToolSpec",
]


def __getattr__(name: str):
    """Lazy import to avoid requiring all dependencies at import time."""
    if name in ("SkillLiteTool", "SkillLiteToolkit"):
        try:
            from .langchain import SkillLiteTool, SkillLiteToolkit
            if name == "SkillLiteTool":
                return SkillLiteTool
            return SkillLiteToolkit
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

