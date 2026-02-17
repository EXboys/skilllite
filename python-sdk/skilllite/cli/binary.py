"""
Binary management commands for skilllite CLI.

Commands: install, uninstall, status, version
"""

import argparse
import sys

from .. import __version__
from ..sandbox.core import (
    BINARY_VERSION,
    get_platform,
    install,
    is_installed,
    get_installed_version,
    uninstall,
)


def print_status() -> None:
    """Print installation status."""
    from ..sandbox.core import find_binary, get_binary_path

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

