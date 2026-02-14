"""
File extension tools - read_file, write_file, write_output, list_directory, file_exists.
"""

from ..builtin_tools import create_builtin_tool_executor, get_file_tools


def register(registry, ctx, executor=None) -> None:
    """Register file tools to the registry."""
    if executor is None:
        executor = create_builtin_tool_executor(
            run_command_confirmation=ctx.confirmation_callback,
            workspace_root=ctx.workspace_root,
            output_root=ctx.output_root,
        )
    for tool_def in get_file_tools():
        name = tool_def.get("function", {}).get("name", "")
        if name:
            registry.register(name, tool_def, executor)
