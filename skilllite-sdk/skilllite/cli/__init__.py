"""
Command-line interface for skilllite.

Provides commands for managing the skillbox binary, similar to
how Playwright provides `playwright install` for browser management.

Usage:
    skilllite install       # Download and install the sandbox binary
    skilllite uninstall     # Remove the installed binary
    skilllite status        # Show installation status
    skilllite version       # Show version information
    skilllite mcp           # Start MCP server
    skilllite init          # Initialize SkillLite project (binary + .skills + deps)
    skilllite init-opencode # Initialize OpenCode integration
"""

from .main import main, create_parser

__all__ = ["main", "create_parser"]

