"""
Extensions - Centralized built-in modules for SkillLite.

All built-in tools and utilities: file, memory, command, long_text.
Add new extensions by creating a module and calling register() from register_extensions().
"""

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Optional

from . import command as command_tools
from . import file as file_tools
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
    Register all extension tools to the registry.

    Args:
        registry: ToolRegistry instance
        ctx: ExtensionsContext with workspace paths and callbacks
        enable_file_tools: Whether to register file tools
        enable_memory_tools: Whether to register memory tools
        enable_command_tools: Whether to register command tools (run_command, preview_server)
    """
    # File and command share the same executor (builtin_tools)
    executor = None
    if enable_file_tools or enable_command_tools:
        from ..builtin_tools import create_builtin_tool_executor

        executor = create_builtin_tool_executor(
            run_command_confirmation=ctx.confirmation_callback,
            workspace_root=ctx.workspace_root,
            output_root=ctx.output_root,
        )

    if enable_file_tools:
        file_tools.register(registry, ctx, executor)
    if enable_command_tools:
        command_tools.register(registry, ctx, executor)
    if enable_memory_tools:
        memory_tools.register(registry, ctx)
