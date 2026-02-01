"""
MCP server command for skilllite CLI.
"""

import argparse
import sys


def cmd_mcp_server(args: argparse.Namespace) -> int:
    """Start MCP server."""
    try:
        import asyncio
        from ..mcp.server import main as mcp_main

        asyncio.run(mcp_main())
        return 0
    except ImportError as e:
        print("Error: MCP integration not available", file=sys.stderr)
        print("Please install it with: pip install skilllite[mcp]", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\nMCP server stopped by user", file=sys.stderr)
        return 0
    except Exception as e:
        import traceback
        print(f"Error starting MCP server: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1

