"""
Command-line interface for skilllite.

Provides commands for managing the skillbox binary, similar to
how Playwright provides `playwright install` for browser management.

Usage:
    skilllite install     # Download and install the sandbox binary
    skilllite uninstall   # Remove the installed binary
    skilllite status      # Show installation status
    skilllite version     # Show version information
    skilllite mcp         # Start MCP server
"""

import argparse
import sys
from typing import List, Optional

from . import __version__
from .sandbox.skillbox import (
    BINARY_VERSION,
    get_platform,
    install,
    is_installed,
    get_installed_version,
    uninstall,
)


def print_status() -> None:
    """Print installation status."""
    from .sandbox.skillbox import find_binary, get_binary_path
    
    print("SkillLite Installation Status")
    print("=" * 40)
    
    if is_installed():
        version = get_installed_version()
        print(f"✓ skillbox is installed (v{version})")
        print(f"  Location: {get_binary_path()}")
    else:
        binary = find_binary()
        if binary:
            print(f"✓ skillbox found at: {binary}")
        else:
            print("✗ skillbox is not installed")
            print("  Install with: skilllite install")

def cmd_install(args: argparse.Namespace) -> int:
    """Install the skillbox binary."""
    try:
        install(
            version=args.version,
            force=args.force,
            show_progress=not args.quiet
        )
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_uninstall(args: argparse.Namespace) -> int:
    """Uninstall the skillbox binary."""
    try:
        uninstall()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_status(args: argparse.Namespace) -> int:
    """Show installation status."""
    try:
        print_status()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_version(args: argparse.Namespace) -> int:
    """Show version information."""
    print(f"skilllite Python SDK: v{__version__}")
    print(f"skillbox binary (bundled): v{BINARY_VERSION}")
    
    installed_version = get_installed_version()
    if installed_version:
        print(f"skillbox binary (installed): v{installed_version}")
    else:
        print("skillbox binary (installed): not installed")
    
    try:
        plat = get_platform()
        print(f"Platform: {plat}")
    except RuntimeError as e:
        print(f"Platform: {e}")
    
    return 0

def cmd_mcp_server(args: argparse.Namespace) -> int:
    """Start MCP server."""
    try:
        import asyncio
        from .mcp.server import main as mcp_main
        
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

def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="skilllite",
        description="SkillLite - A lightweight Skills execution engine with LLM integration",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  skilllite install          Install the sandbox binary
  skilllite install --force  Force reinstall
  skilllite status           Check installation status
  skilllite uninstall        Remove the binary
  skilllite mcp              Start MCP server (requires pip install skilllite[mcp])

For more information, visit: https://github.com/skilllite/skilllite
        """
    )
    
    parser.add_argument(
        "-V", "--version",
        action="store_true",
        help="Show version information"
    )
    
    subparsers = parser.add_subparsers(dest="command", help="Available commands")
    
    # install command
    install_parser = subparsers.add_parser(
        "install",
        help="Download and install the skillbox sandbox binary"
    )
    install_parser.add_argument(
        "--version",
        dest="version",
        default=None,
        help=f"Version to install (default: {BINARY_VERSION})"
    )
    install_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force reinstall even if already installed"
    )
    install_parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress progress output"
    )
    install_parser.set_defaults(func=cmd_install)
    
    # uninstall command
    uninstall_parser = subparsers.add_parser(
        "uninstall",
        help="Remove the installed skillbox binary"
    )
    uninstall_parser.set_defaults(func=cmd_uninstall)
    
    # status command
    status_parser = subparsers.add_parser(
        "status",
        help="Show installation status"
    )
    status_parser.set_defaults(func=cmd_status)
    
    # version command (alternative to -V)
    version_parser = subparsers.add_parser(
        "version",
        help="Show version information"
    )
    version_parser.set_defaults(func=cmd_version)
    
    # mcp command
    mcp_parser = subparsers.add_parser(
        "mcp",
        help="Start MCP server for SkillLite"
    )
    mcp_parser.set_defaults(func=cmd_mcp_server)
    
    return parser

def main(argv: Optional[List[str]] = None) -> int:
    """Main entry point for the CLI."""
    parser = create_parser()
    args = parser.parse_args(argv)
    
    # Handle -V/--version flag
    if args.version:
        return cmd_version(args)
    
    # Handle no command
    if not args.command:
        parser.print_help()
        return 0
    
    # Execute the command
    return args.func(args)

if __name__ == "__main__":
    sys.exit(main())
