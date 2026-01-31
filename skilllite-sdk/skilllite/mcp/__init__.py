"""
MCP (Model Context Protocol) Integration for SkillLite.

This module provides MCP server functionality for SkillLite, allowing
you to use SkillLite as an MCP tool server.

Example:
    ```python
    from skilllite.mcp import MCPServer
    
    # Start MCP server
    server = MCPServer()
    await server.run()
    ```

Or via CLI:
    ```bash
    skilllite mcp server
    ```
"""

# Check MCP availability
try:
    from mcp.server import Server
    MCP_AVAILABLE = True
except ImportError:
    MCP_AVAILABLE = False

# Lazy imports to avoid module loading issues when running with -m
def __getattr__(name):
    if name == "MCPServer":
        from .server import MCPServer
        return MCPServer
    elif name == "SandboxExecutor":
        from .server import SandboxExecutor
        return SandboxExecutor
    elif name == "MCP_AVAILABLE":
        return MCP_AVAILABLE
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")

__all__ = [
    "MCPServer",
    "SandboxExecutor",
    "MCP_AVAILABLE",
]
