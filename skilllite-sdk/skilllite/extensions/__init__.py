"""
Extensions - Built-in modules for SkillLite.

File and command tools are now provided by skillbox (Rust) agent-rpc.
Memory tools and build_memory_context remain for SDK use.
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Optional

from . import memory as memory_tools


@dataclass
class ExtensionsContext:
    """Context for extension tool registration."""

    workspace_root: Path
    output_root: Path
    workspace_path: str
    confirmation_callback: Optional[Callable[[str, str], bool]] = None


def register_extensions(
    registry: Any,
    ctx: ExtensionsContext,
    enable_file_tools: bool = True,
    enable_memory_tools: bool = True,
    enable_command_tools: bool = True,
) -> None:
    """
    Register extension tools. File/command tools are now in skillbox agent-rpc.
    Only memory tools are registered here (when using custom agent loops).
    """
    if enable_memory_tools:
        memory_tools.register(registry, ctx)
